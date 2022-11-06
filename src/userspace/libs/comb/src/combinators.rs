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
    I: PartialEq + core::fmt::Debug + Clone,
    E: Error,
{
    type Error = E;
    type Output = I;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (next, span) = stream.next().ok_or_else(|| E::unexpected_end_of_input())?;

        match next == self.input {
            true => Ok(next),
            false => Err(E::expected_one_of(next, &[self.input.clone()], Some(span))),
        }
    }
}

#[inline]
pub const fn single_by<I, E, F>(f: F) -> SingleBy<F, I, E>
where
    I: PartialEq + Clone,
    E: Error,
    F: Fn(&I) -> bool,
{
    SingleBy { f, _i: core::marker::PhantomData, _e: core::marker::PhantomData }
}

pub struct SingleBy<F, I, E> {
    f: F,
    _i: core::marker::PhantomData<I>,
    _e: core::marker::PhantomData<E>,
}

impl<F, I, E> Clone for SingleBy<F, I, E>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self { f: self.f.clone(), _i: core::marker::PhantomData, _e: core::marker::PhantomData }
    }
}

impl<F, I, E> Parser for SingleBy<F, I, E>
where
    F: Fn(&I) -> bool,
    I: PartialEq + core::fmt::Debug + Clone,
    E: Error,
{
    type Error = E;
    type Output = I;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (next, span) = stream.next().ok_or_else(|| E::custom("end of stream", None))?;

        match (self.f)(&next) {
            true => Ok(next),
            false => Err(E::unexpected_value(next, Some(span))),
        }
    }
}

#[inline]
pub const fn any<I: PartialEq + Clone, E: Error>() -> Any<I, E> {
    Any { _i: core::marker::PhantomData, _e: core::marker::PhantomData }
}

pub struct Any<I, E> {
    _i: core::marker::PhantomData<I>,
    _e: core::marker::PhantomData<E>,
}

impl<I, E> Parser for Any<I, E>
where
    I: PartialEq + core::fmt::Debug + Clone,
    E: Error,
{
    type Error = E;
    type Output = I;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        Ok(stream.next().ok_or_else(|| E::custom("end of stream", None))?.0)
    }
}

#[inline]
pub const fn maybe<E, I, O, P>(parser: P) -> Maybe<E, I, O, P>
where
    E: Error,
    I: PartialEq + Clone,
    P: Parser<Error = E, Input = I, Output = O>,
{
    Maybe { parser }
}

pub struct Maybe<E, I, O, P>
where
    E: Error,
    I: PartialEq + Clone,
    P: Parser<Error = E, Input = I, Output = O>,
{
    parser: P,
}

impl<E, I, O, P> Parser for Maybe<E, I, O, P>
where
    E: Error,
    I: core::fmt::Debug + PartialEq + Clone,
    P: Parser<Error = E, Input = I, Output = O>,
{
    type Error = E;
    type Output = Option<O>;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        Ok(self.parser.try_parse(stream).ok())
    }
}

#[inline]
pub const fn one_of<I, E, C>(input: C) -> OneOf<I, E, C>
where
    I: PartialEq + core::fmt::Debug + choice::Hint<C>,
    E: Error,
{
    OneOf { input, _e: core::marker::PhantomData, _i: core::marker::PhantomData }
}

pub struct OneOf<I, E, C>
where
    I: PartialEq + core::fmt::Debug + choice::Hint<C>,
    E: Error,
{
    input: C,
    _e: core::marker::PhantomData<E>,
    _i: core::marker::PhantomData<I>,
}

impl<I, E, C> Clone for OneOf<I, E, C>
where
    I: PartialEq + core::fmt::Debug + choice::Hint<C>,
    E: Error,
    C: Clone,
{
    fn clone(&self) -> Self {
        Self { input: self.input.clone(), _e: core::marker::PhantomData, _i: core::marker::PhantomData }
    }
}

impl<I, E, C> Parser for OneOf<I, E, C>
where
    I: PartialEq + core::fmt::Debug + choice::Hint<C> + Clone,
    E: Error,
{
    type Error = E;
    type Output = I;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (next, span) = stream.next().ok_or_else(|| E::custom("end of stream", None))?;

        match choice::Hint::is_hinted(&next, &self.input) {
            true => Ok(next),
            false => Err(E::unexpected_value(next, Some(span))),
        }
    }
}

#[inline]
pub const fn sequence<I, E>(sequence: &[I]) -> Sequence<'_, I, E>
where
    I: PartialEq,
    E: Error,
{
    Sequence { sequence, _e: core::marker::PhantomData }
}

pub struct Sequence<'a, I, E> {
    sequence: &'a [I],
    _e: core::marker::PhantomData<E>,
}

impl<I, E> Clone for Sequence<'_, I, E> {
    fn clone(&self) -> Self {
        Self { sequence: self.sequence, _e: core::marker::PhantomData }
    }
}

impl<I, E> Parser for Sequence<'_, I, E>
where
    I: PartialEq + core::fmt::Debug + Clone,
    E: Error,
{
    type Error = E;
    type Output = ();
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let mut entire_span = None;
        for item in self.sequence {
            let (next, span) = stream.next().ok_or_else(|| E::custom("end of stream", None))?;

            match &mut entire_span {
                entire_span @ None => *entire_span = Some(span),
                Some(entire_span) => entire_span.end = span.end,
            }

            if item != &next {
                return Err(E::expected_one_of(next, self.sequence, Some(span)));
            }
        }

        Ok(())
    }
}

#[inline]
pub const fn choice<I: PartialEq + Clone, O, E: Error, P>(subparsers: P) -> Choice<I, O, E, P> {
    Choice { subparsers, _i: core::marker::PhantomData, _o: core::marker::PhantomData, _e: core::marker::PhantomData }
}

#[inline]
pub const fn hinted_choice<I: Clone, O, E: Error, P>(subparsers: P) -> HintedChoice<I, O, E, P> {
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
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    Peek { parser }
}

pub struct Peek<I, O, E, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: P,
}

impl<I, O, E, P> Clone for Peek<I, O, E, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { parser: self.parser.clone() }
    }
}

impl<I, O, E, P> Parser for Peek<I, O, E, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    type Error = E;
    type Output = O;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.parser.try_parse(stream)
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
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
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
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
    O: core::fmt::Debug,
{
    type Error = E;
    type Output = alloc::vec::Vec<O>;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let mut collection = alloc::vec::Vec::new();

        while let Ok(parsed) = self.parser.try_parse(stream) {
            collection.push(parsed);
        }

        Ok(collection)
    }
}

#[inline]
pub const fn many1<I, O, E, P>(parser: P) -> Many0<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    Many0 { parser }
}

pub struct Many1<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: P,
}

impl<I, O, E, P> Clone for Many1<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { parser: self.parser.clone() }
    }
}

impl<I, O, E, P> Parser for Many1<I, O, E, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    type Error = E;
    type Output = alloc::vec::Vec<O>;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let first = self.parser.parse(stream)?;
        let mut collection = alloc::vec![first];

        while let Ok(parsed) = self.parser.try_parse(stream) {
            collection.push(parsed);
        }

        Ok(collection)
    }
}

pub fn end<I, E>() -> End<I, E>
where
    I: core::fmt::Debug + Clone,
    E: Error,
{
    End { _i: core::marker::PhantomData, _e: core::marker::PhantomData }
}

pub struct End<I, E> {
    _i: core::marker::PhantomData<I>,
    _e: core::marker::PhantomData<E>,
}

impl<I, E> Parser for End<I, E>
where
    I: core::fmt::Debug + Clone,
    E: Error,
{
    type Error = E;
    type Input = I;
    type Output = ();

    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        match stream.next() {
            Some((next, span)) => Err(E::unexpected_value(next, Some(span))),
            None => Ok(()),
        }
    }
}

pub fn consume<E, I, O, P>(parser: P) -> Consume<E, I, O, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    Consume { parser }
}

pub struct Consume<E, I, O, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    parser: P,
}

impl<E, I, O, P> Parser for Consume<E, I, O, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    type Error = E;
    type Input = I;
    type Output = ();

    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.parser.parse(stream)?;
        Ok(())
    }
}

pub fn consume_many<E, I, O, P>(parser: P) -> ConsumeMany<E, I, O, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    ConsumeMany { parser }
}

pub struct ConsumeMany<E, I, O, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    parser: P,
}

impl<E, I, O, P> Parser for ConsumeMany<E, I, O, P>
where
    I: core::fmt::Debug + Clone,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    type Error = E;
    type Input = I;
    type Output = ();

    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        while self.parser.try_parse(stream).is_ok() {}
        Ok(())
    }
}

pub fn until<C, E, I, O, P>(hint: C, parser: P) -> Until<C, E, I, O, P>
where
    I: core::fmt::Debug + Clone + choice::Hint<C>,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    Until { hint, parser }
}

pub struct Until<C, E, I, O, P>
where
    I: core::fmt::Debug + Clone + choice::Hint<C>,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    hint: C,
    parser: P,
}

impl<C, E, I, O, P> Parser for Until<C, E, I, O, P>
where
    I: core::fmt::Debug + Clone + choice::Hint<C>,
    E: Error,
    P: Parser<Error = E, Input = I, Output = O>,
{
    type Error = E;
    type Input = I;
    type Output = alloc::vec::Vec<O>;

    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let mut values = alloc::vec::Vec::new();

        loop {
            let peek = stream.peek().ok_or_else(|| E::unexpected_end_of_input())?;
            if peek.0.is_hinted(&self.hint) {
                break;
            }
            values.push(self.parser.parse(stream)?);
        }

        Ok(values)
    }
}
