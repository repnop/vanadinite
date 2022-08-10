// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

// #![feature(generic_associated_types)]
#![no_std]

extern crate alloc;

pub mod combinators;
pub mod error;
pub mod stream;
pub mod utils;

use error::Error;
use stream::Stream;

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl core::fmt::Display for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

pub trait Parser {
    type Error: Error;
    type Output;
    type Input: core::fmt::Debug;

    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>;
    fn try_parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Clone,
        S: Stream<Item = Self::Input>,
    {
        let mut original = stream.clone();
        let ret = self.parse(&mut original);

        if ret.is_ok() {
            core::mem::swap(stream, &mut original);
        }

        ret
    }

    fn map<U, F: Fn(Self::Output) -> U>(self, f: F) -> Map<Self, U, F, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
    {
        Map {
            p: self,
            f,
            _e: core::marker::PhantomData,
            _i: core::marker::PhantomData,
            _u: core::marker::PhantomData,
            _o: core::marker::PhantomData,
        }
    }

    fn to<U>(self, value: U) -> To<Self, U, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        U: Clone,
    {
        To {
            p: self,
            value,
            _e: core::marker::PhantomData,
            _s: core::marker::PhantomData,
            _u: core::marker::PhantomData,
            _o: core::marker::PhantomData,
        }
    }
}

impl<P: Parser> Parser for &'_ P {
    type Error = P::Error;
    type Output = P::Output;
    type Input = P::Input;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        (*self).parse(stream)
    }
}

pub struct Map<P, U, F, E, O, I>
where
    F: Fn(O) -> U,
{
    p: P,
    f: F,
    _e: core::marker::PhantomData<E>,
    _i: core::marker::PhantomData<I>,
    _u: core::marker::PhantomData<U>,
    _o: core::marker::PhantomData<O>,
}

impl<P, U, F, E, O, I> Parser for Map<P, U, F, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    F: Fn(O) -> U,
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Output = U;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        self.p.parse(stream).map(&self.f)
    }
}

pub struct To<P, U, E, O, S> {
    p: P,
    value: U,
    _e: core::marker::PhantomData<E>,
    _s: core::marker::PhantomData<S>,
    _u: core::marker::PhantomData<U>,
    _o: core::marker::PhantomData<O>,
}

impl<P, U, E, O, I> Parser for To<P, U, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    U: Clone,
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Output = U;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        self.p.parse(stream)?;
        Ok(self.value.clone())
    }
}

// #[inline]
// pub fn custom<E, I, O, F>(f: F) -> Custom<E, I, O, F>
// where
//     E: Error,
//     F: Fn(&mut dyn Stream<Item = I>) -> Result<O, E>,
// {
//     Custom { f, _e: core::marker::PhantomData, _i: core::marker::PhantomData }
// }

// pub struct Custom<E, I, O, F>
// where
//     E: Error,
//     F: Fn(&mut dyn Stream<Item = I>) -> Result<O, E>,
// {
//     f: F,
//     _e: core::marker::PhantomData<E>,
//     _i: core::marker::PhantomData<I>,
// }

// impl<E, I, O, F> Parser for Custom<E, I, O, F>
// where
//     E: Error,
//     F: Fn(&mut dyn Stream<Item = I>) -> Result<O, E>,
//     I: core::fmt::Debug,
// {
//     type Error = E;
//     type Output = O;
//     type Input = I;

//     #[inline]
//     fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
//     where
//         S: Stream<Item = Self::Input>,
//     {
//         (self.f)(stream)
//     }
// }
