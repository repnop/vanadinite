// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(let_else)]
#![no_std]

extern crate alloc;

pub mod combinators;
pub mod error;
pub mod recursive;
pub mod stream;
pub mod text;
pub mod utils;

use error::Error;
use stream::Stream;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
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

    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error>;

    fn try_parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error>
    where
        Self::Input: Clone,
    {
        stream.try_parse(|stream| self.parse(stream))
    }

    fn map<U, F: Fn(Self::Output) -> U>(self, f: F) -> Map<Self, U, F, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
    {
        Map { p: self, f }
    }

    fn to<U>(self, value: U) -> To<Self, U, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        U: Clone,
    {
        To { p: self, value }
    }

    fn or<P>(self, parser: P) -> Or<Self, P, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        P: Parser<Error = Self::Error, Output = Self::Output, Input = Self::Input>,
    {
        Or { left: self, right: parser }
    }

    fn then<O, P>(self, parser: P) -> Then<Self, P, O, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        P: Parser<Error = Self::Error, Output = O, Input = Self::Input>,
    {
        Then { left: self, right: parser }
    }

    fn then_to<O, P>(self, parser: P) -> ThenTo<Self, P, O, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        P: Parser<Error = Self::Error, Output = O, Input = Self::Input>,
    {
        ThenTo { left: self, right: parser }
    }

    fn then_assert<O, P>(self, parser: P) -> ThenAssert<Self, O, P, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        P: Parser<Error = Self::Error, Output = O, Input = Self::Input>,
    {
        ThenAssert { parser: self, tail: parser }
    }

    fn padded_by<O, P>(self, padding: P) -> PaddedBy<Self, P, O, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        P: Parser<Error = Self::Error, Output = O, Input = Self::Input>,
    {
        PaddedBy { padding, parser: self }
    }

    fn separated_by<O, P>(self, separator: P) -> SeparatedBy<Self, P, O, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
        P: Parser<Error = Self::Error, Output = O, Input = Self::Input>,
    {
        SeparatedBy { separator, parser: self, trailing: false }
    }

    fn with_span(self) -> WithSpan<Self, Self::Error, Self::Output, Self::Input>
    where
        Self: Sized,
    {
        WithSpan { parser: self }
    }
}

impl<P: Parser> Parser for &'_ P {
    type Error = P::Error;
    type Output = P::Output;
    type Input = P::Input;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        (*self).parse(stream)
    }
}

impl<E, I, O> Parser for &'_ dyn Parser<Error = E, Input = I, Output = O>
where
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Output = O;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        (*self).parse(stream)
    }
}

pub struct Map<P, U, F, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    F: Fn(O) -> U,
    E: Error,
    I: core::fmt::Debug,
{
    p: P,
    f: F,
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
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.p.parse(stream).map(|val| (self.f)(val))
    }
}

pub struct To<P, U, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    U: Clone,
    E: Error,
    I: core::fmt::Debug,
{
    p: P,
    value: U,
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
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.p.parse(stream)?;
        Ok(self.value.clone())
    }
}

pub struct Then<L, R, O2, E, O, I>
where
    L: Parser<Error = E, Output = O, Input = I>,
    R: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    left: L,
    right: R,
}

impl<L, R, O2, E, O, I> Parser for Then<L, R, O2, E, O, I>
where
    L: Parser<Error = E, Output = O, Input = I>,
    R: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Output = (O, O2);
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let first = self.left.parse(stream)?;
        let second = self.right.parse(stream)?;
        Ok((first, second))
    }
}

pub struct ThenTo<L, R, O2, E, O, I>
where
    L: Parser<Error = E, Output = O, Input = I>,
    R: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    left: L,
    right: R,
}

impl<L, R, O2, E, O, I> Parser for ThenTo<L, R, O2, E, O, I>
where
    L: Parser<Error = E, Output = O, Input = I>,
    R: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Output = O2;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.left.parse(stream)?;
        self.right.parse(stream)
    }
}

pub struct Or<L, R, E, O, I>
where
    L: Parser<Error = E, Output = O, Input = I>,
    R: Parser<Error = E, Output = O, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    left: L,
    right: R,
}

impl<L, R, E, O, I> Parser for Or<L, R, E, O, I>
where
    L: Parser<Error = E, Output = O, Input = I>,
    R: Parser<Error = E, Output = O, Input = I>,
    E: Error,
    I: core::fmt::Debug + Clone,
{
    type Error = E;
    type Output = O;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.left.try_parse(stream).or_else(|_| self.right.parse(stream))
    }
}

pub struct ThenAssert<P, O2, T, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    T: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    parser: P,
    tail: T,
}

impl<P, O2, T, E, O, I> Parser for ThenAssert<P, O2, T, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    T: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Output = O;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let parsed = self.parser.parse(stream)?;
        self.tail.parse(stream)?;
        Ok(parsed)
    }
}

pub struct PaddedBy<P, PD, O2, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    PD: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    parser: P,
    padding: PD,
}

impl<P, PD, O2, E, O, I> Parser for PaddedBy<P, PD, O2, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    PD: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug + Clone,
{
    type Error = E;
    type Output = O;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        while self.padding.try_parse(stream).is_ok() {}
        self.parser.parse(stream)
    }
}

pub struct SeparatedBy<P, S, O2, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    S: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    parser: P,
    separator: S,
    trailing: bool,
}

impl<P, S, O2, E, O, I> SeparatedBy<P, S, O2, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    S: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    pub fn allow_trailing(mut self) -> Self {
        self.trailing = true;
        self
    }
}

impl<P, PD, O2, E, O, I> Parser for SeparatedBy<P, PD, O2, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    PD: Parser<Error = E, Output = O2, Input = I>,
    E: Error,
    I: core::fmt::Debug + Clone,
{
    type Error = E;
    type Output = alloc::vec::Vec<O>;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let mut values = alloc::vec![self.parser.parse(stream)?];
        if self.separator.try_parse(stream).is_err() {
            return Ok(values);
        }

        loop {
            match self.trailing {
                false => values.push(self.parser.parse(stream)?),
                true => match self.parser.try_parse(stream) {
                    Ok(value) => values.push(value),
                    Err(_) => break,
                },
            }

            if self.separator.try_parse(stream).is_err() {
                break;
            }
        }

        Ok(values)
    }
}

pub struct WithSpan<P, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    E: Error,
    I: core::fmt::Debug,
{
    parser: P,
}

impl<P, E, O, I> Parser for WithSpan<P, E, O, I>
where
    P: Parser<Error = E, Output = O, Input = I>,
    E: Error,
    I: core::fmt::Debug + Clone,
{
    type Error = E;
    type Output = (O, Span);
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        stream.begin_record_span();
        match self.parser.parse(stream) {
            Ok(val) => {
                let span = stream.end_record_span().unwrap_or_default();
                Ok((val, span))
            }
            Err(e) => {
                stream.end_record_span();
                Err(e)
            }
        }
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
