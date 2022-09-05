// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Error, Parser};

pub fn whitespace<E: Error>() -> Whitespace<E> {
    Whitespace(core::marker::PhantomData)
}

pub struct Whitespace<E>(core::marker::PhantomData<fn() -> E>);

impl<E> Parser for Whitespace<E>
where
    E: Error,
{
    type Error = E;
    type Input = char;
    type Output = char;

    fn parse(&self, stream: &mut crate::stream::Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (c, span) = stream.next().ok_or_else(|| E::unexpected_end_of_input())?;

        match c.is_ascii_whitespace() {
            true => Ok(c),
            false => match stream.in_try_mode() {
                false => Err(E::expected_one_of(c, &[' ', '\n', '\t', '\r'], Some(span))),
                true => Err(E::hopefully_cheap()),
            },
        }
    }
}

pub fn ascii_alphabetic<E: Error>() -> AsciiAlphabetic<E> {
    AsciiAlphabetic(core::marker::PhantomData)
}

pub struct AsciiAlphabetic<E>(core::marker::PhantomData<fn() -> E>);

impl<E> Parser for AsciiAlphabetic<E>
where
    E: Error,
{
    type Error = E;
    type Input = char;
    type Output = char;

    fn parse(&self, stream: &mut crate::stream::Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (c, span) = stream.next().ok_or_else(|| E::unexpected_end_of_input())?;

        match c.is_ascii_alphabetic() {
            true => Ok(c),
            false => match stream.in_try_mode() {
                false => Err(E::unexpected_value(c, Some(span))),
                true => Err(E::hopefully_cheap()),
            },
        }
    }
}

pub fn ascii_digit<E: Error>() -> AsciiDigit<E> {
    AsciiDigit(core::marker::PhantomData)
}

pub struct AsciiDigit<E>(core::marker::PhantomData<fn() -> E>);

impl<E> Parser for AsciiDigit<E>
where
    E: Error,
{
    type Error = E;
    type Input = char;
    type Output = char;

    fn parse(&self, stream: &mut crate::stream::Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (c, span) = stream.next().ok_or_else(|| E::unexpected_end_of_input())?;

        match c.is_ascii_digit() {
            true => Ok(c),
            false => match stream.in_try_mode() {
                false => Err(E::expected_one_of(c, &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'], Some(span))),
                true => Err(E::hopefully_cheap()),
            },
        }
    }
}

pub fn ascii_alphanumeric<E: Error>() -> AsciiAlphanumeric<E> {
    AsciiAlphanumeric(core::marker::PhantomData)
}

pub struct AsciiAlphanumeric<E>(core::marker::PhantomData<fn() -> E>);

impl<E> Parser for AsciiAlphanumeric<E>
where
    E: Error,
{
    type Error = E;
    type Input = char;
    type Output = char;

    fn parse(&self, stream: &mut crate::stream::Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let (c, span) = stream.next().ok_or_else(|| E::unexpected_end_of_input())?;

        match c.is_ascii_alphanumeric() {
            true => Ok(c),
            false => match stream.in_try_mode() {
                false => Err(E::unexpected_value(c, Some(span))),
                true => Err(E::hopefully_cheap()),
            },
        }
    }
}

pub trait StringParser<E: Error> {
    fn head(&self) -> Option<&dyn Parser<Error = E, Input = char, Output = char>>;
    fn mid(&self) -> Option<&dyn Parser<Error = E, Input = char, Output = char>>;
}

impl<E, P> StringParser<E> for P
where
    E: Error,
    P: Parser<Error = E, Input = char, Output = char>,
{
    fn head(&self) -> Option<&dyn Parser<Error = E, Input = char, Output = char>> {
        None
    }

    fn mid(&self) -> Option<&dyn Parser<Error = E, Input = char, Output = char>> {
        Some(self as _)
    }
}

impl<E, PH, PT> StringParser<E> for (PH, PT)
where
    E: Error,
    PH: Parser<Error = E, Input = char, Output = char>,
    PT: Parser<Error = E, Input = char, Output = char>,
{
    fn head(&self) -> Option<&dyn Parser<Error = E, Input = char, Output = char>> {
        Some(&self.0 as _)
    }

    fn mid(&self) -> Option<&dyn Parser<Error = E, Input = char, Output = char>> {
        Some(&self.1 as _)
    }
}

pub fn string<E: Error, P: StringParser<E>>(parser: P) -> String<E, P> {
    String { parser, _e: core::marker::PhantomData }
}

pub struct String<E: Error, P: StringParser<E>> {
    parser: P,
    _e: core::marker::PhantomData<fn() -> E>,
}

impl<E, P> Parser for String<E, P>
where
    E: Error,
    P: StringParser<E>,
{
    type Error = E;
    type Input = char;
    type Output = alloc::string::String;

    fn parse(&self, stream: &mut crate::stream::Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        let mut str = alloc::string::String::new();

        match (self.parser.head(), self.parser.mid()) {
            (Some(head), Some(mid)) => {
                let first = head.parse(stream)?;
                str.push(first);

                while let Ok(next) = mid.try_parse(stream) {
                    str.push(next);
                }

                Ok(str)
            }
            (None, Some(mid)) => {
                let first = mid.parse(stream)?;
                str.push(first);

                while let Ok(next) = mid.try_parse(stream) {
                    str.push(next);
                }

                Ok(str)
            }
            _ => unreachable!(),
        }
    }
}
