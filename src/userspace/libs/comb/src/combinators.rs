// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod choice;

use crate::{Error, Parser, Stream};
use choice::{Choice, HintedChoice};

#[inline]
pub const fn single<I: PartialEq + Clone, E: Error>(input: I) -> Single<I, E> {
    Single { input, _i: core::marker::PhantomData, _e: core::marker::PhantomData }
}

pub struct Single<I, E> {
    input: I,
    _i: core::marker::PhantomData<I>,
    _e: core::marker::PhantomData<E>,
}

impl<I, E> Clone for Single<I, E>
where
    I: Clone,
{
    fn clone(&self) -> Self {
        Self { input: self.input.clone(), _i: core::marker::PhantomData, _e: core::marker::PhantomData }
    }
}

impl<I, E> Parser for Single<I, E>
where
    I: PartialEq + core::fmt::Debug,
    E: Error,
{
    type Error = E;
    type Output = I;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        let (next, span) = stream.next().ok_or_else(|| E::custom("end of stream", None))?;

        match next == self.input {
            true => Ok(next),
            false => Err(E::custom("mismatched values", Some(span))),
        }
    }
}

#[inline]
pub const fn one_of<I, E>(input: &[I]) -> OneOf<'_, I, E>
where
    I: PartialEq,
    E: Error,
{
    OneOf { input, _e: core::marker::PhantomData }
}

pub struct OneOf<'a, I, E> {
    input: &'a [I],
    _e: core::marker::PhantomData<E>,
}

impl<I, E> Clone for OneOf<'_, I, E> {
    fn clone(&self) -> Self {
        Self { input: self.input, _e: core::marker::PhantomData }
    }
}

impl<I, E> Parser for OneOf<'_, I, E>
where
    I: PartialEq + core::fmt::Debug,
    E: Error,
{
    type Error = E;
    type Output = I;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        let (next, span) = stream.next().ok_or_else(|| E::custom("end of stream", None))?;

        match self.input.contains(&next) {
            true => Ok(next),
            false => Err(E::expected_one_of(next, self.input, Some(span))),
        }
    }
}

#[inline]
pub const fn choice<I: PartialEq + Clone, O, E: Error, P>(subparsers: P) -> Choice<I, O, E, P> {
    Choice { subparsers, _i: core::marker::PhantomData, _o: core::marker::PhantomData, _e: core::marker::PhantomData }
}

#[inline]
pub const fn hinted_choice<I: PartialEq + Clone, O, E: Error, P>(subparsers: P) -> HintedChoice<I, O, E, P> {
    HintedChoice {
        subparsers,
        _i: core::marker::PhantomData,
        _o: core::marker::PhantomData,
        _e: core::marker::PhantomData,
    }
}

#[inline]
pub const fn peek<I, O, E, P>(parser: P) -> Peek<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    Peek { parser }
}

pub struct Peek<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: P,
}

impl<I, O, E, P> Clone for Peek<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { parser: self.parser.clone() }
    }
}

impl<I, O, E, P> Parser for Peek<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    type Error = E;
    type Output = O;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        let mut original = stream.clone();
        self.parser.parse(&mut original)
    }
}

pub const fn delimited<I, O1, O2, O3, E, L, P, R>(start: L, parser: P, end: R) -> Delimited<I, O1, O2, O3, E, L, P, R>
where
    I: PartialEq + core::fmt::Debug,
    E: Error,
    L: Parser<Error = E, Output = O1, Input = I>,
    P: Parser<Error = E, Output = O2, Input = I>,
    R: Parser<Error = E, Output = O3, Input = I>,
{
    Delimited { start, parser, end }
}

pub struct Delimited<I, O1, O2, O3, E, L, P, R>
where
    I: PartialEq + core::fmt::Debug,
    E: Error,
    L: Parser<Error = E, Output = O1, Input = I>,
    P: Parser<Error = E, Output = O2, Input = I>,
    R: Parser<Error = E, Output = O3, Input = I>,
{
    start: L,
    parser: P,
    end: R,
}

impl<I, O1, O2, O3, E, L, P, R> Clone for Delimited<I, O1, O2, O3, E, L, P, R>
where
    I: PartialEq + core::fmt::Debug,
    E: Error,
    L: Parser<Error = E, Output = O1, Input = I> + Clone,
    P: Parser<Error = E, Output = O2, Input = I> + Clone,
    R: Parser<Error = E, Output = O3, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { start: self.start.clone(), parser: self.parser.clone(), end: self.end.clone() }
    }
}

impl<I, O1, O2, O3, E, L, P, R> Parser for Delimited<I, O1, O2, O3, E, L, P, R>
where
    I: PartialEq + core::fmt::Debug,
    E: Error,
    L: Parser<Error = E, Output = O1, Input = I>,
    P: Parser<Error = E, Output = O2, Input = I>,
    R: Parser<Error = E, Output = O3, Input = I>,
{
    type Error = E;
    type Output = O2;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        self.start.parse(stream)?;
        let ret = self.parser.parse(stream)?;
        self.end.parse(stream)?;

        Ok(ret)
    }
}

#[inline]
pub const fn many0<I, O, E, P>(parser: P) -> Many0<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    Many0 { parser }
}

pub struct Many0<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: P,
}

impl<I, O, E, P> Clone for Many0<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { parser: self.parser.clone() }
    }
}

impl<I, O, E, P> Parser for Many0<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    type Error = E;
    type Output = alloc::vec::Vec<O>;
    type Input = I;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        let mut collection = alloc::vec::Vec::new();

        while let Ok(parsed) = self.parser.try_parse(stream) {
            collection.push(parsed);
        }

        Ok(collection)
    }
}
