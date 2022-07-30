// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::marker::PhantomData;

pub struct Parser<'a> {
    pub(crate) state: &'a [u8],
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { state: input }
    }

    pub fn number(&mut self) -> Result<i64, ParseError> {
        i64::parse(self)
    }

    pub(crate) fn parse<T: Parseable<'a>>(&mut self) -> Result<T, ParseError> {
        T::parse(self)
    }

    pub(crate) fn parse_or_rewind<T: Parseable<'a>>(&mut self) -> Option<T> {
        let current = self.state;
        match self.parse::<T>() {
            Ok(t) => Some(t),
            Err(_) => {
                self.state = current;
                None
            }
        }
    }

    pub(crate) fn eat(&mut self, c: char) -> Result<(), ParseError> {
        let next = self.next()?;
        match next == c {
            true => Ok(()),
            false => Err(ParseError::UnexpectedCharacter(next)),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn next(&mut self) -> Result<char, ParseError> {
        self.skip_whitespace();
        match self.state {
            [c, rest @ ..] => {
                self.state = rest;
                Ok(*c as char)
            }
            _ => Err(ParseError::UnexpectedEof),
        }
    }

    pub(crate) fn next_raw(&mut self) -> Result<char, ParseError> {
        match self.state {
            [c, rest @ ..] => {
                self.state = rest;
                Ok(*c as char)
            }
            _ => Err(ParseError::UnexpectedEof),
        }
    }

    pub(crate) fn peek(&mut self) -> Option<char> {
        self.skip_whitespace();
        match self.state {
            [c, ..] => Some(*c as char),
            _ => None,
        }
    }

    fn peek_raw(&mut self) -> Option<char> {
        match self.state {
            [c, ..] => Some(*c as char),
            _ => None,
        }
    }

    pub(crate) fn skip_whitespace(&mut self) {
        while matches!(self.peek_raw(), Some(c) if c.is_ascii_whitespace()) {
            self.next_raw().unwrap();
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    IntegerValueTooLarge,
    InvalidUtf8,
    UnexpectedCharacter(char),
    UnexpectedEof,
}

impl From<core::num::TryFromIntError> for ParseError {
    fn from(_: core::num::TryFromIntError) -> Self {
        Self::IntegerValueTooLarge
    }
}

pub trait Parseable<'a>: Sized {
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError>;
}

pub(super) struct Comma;
impl<'a> Parseable<'a> for Comma {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat(',').map(|_| Self)
    }
}

pub(super) struct Colon;
impl<'a> Parseable<'a> for Colon {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat(':').map(|_| Self)
    }
}

pub(super) struct LeftBrace;
impl<'a> Parseable<'a> for LeftBrace {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat('{').map(|_| Self)
    }
}

pub(super) struct RightBrace;
impl<'a> Parseable<'a> for RightBrace {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat('}').map(|_| Self)
    }
}

pub(super) struct LeftBracket;
impl<'a> Parseable<'a> for LeftBracket {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat('[').map(|_| Self)
    }
}

pub(super) struct RightBracket;
impl<'a> Parseable<'a> for RightBracket {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat(']').map(|_| Self)
    }
}

pub(super) struct Quote;
impl<'a> Parseable<'a> for Quote {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.eat('"').map(|_| Self)
    }
}

impl<'a, T: Parseable<'a>, U: Parseable<'a>> Parseable<'a> for (T, U) {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        Ok((T::parse(parser)?, U::parse(parser)?))
    }
}

impl<'a, T: Parseable<'a>, U: Parseable<'a>, V: Parseable<'a>> Parseable<'a> for (T, U, V) {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        Ok((T::parse(parser)?, U::parse(parser)?, V::parse(parser)?))
    }
}

impl<'a, T: Parseable<'a>, U: Parseable<'a>, V: Parseable<'a>, W: Parseable<'a>> Parseable<'a> for (T, U, V, W) {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        Ok((T::parse(parser)?, U::parse(parser)?, V::parse(parser)?, W::parse(parser)?))
    }
}

impl<'a, T: Parseable<'a>, U: Parseable<'a>, V: Parseable<'a>, W: Parseable<'a>, X: Parseable<'a>> Parseable<'a>
    for (T, U, V, W, X)
{
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        Ok((T::parse(parser)?, U::parse(parser)?, V::parse(parser)?, W::parse(parser)?, X::parse(parser)?))
    }
}

impl<'a> Parseable<'a> for &'a str {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        parser.parse::<Quote>()?;
        let current = parser.state;
        let mut len = 0;
        while parser.next_raw()? != '"' {
            len += 1;
        }

        core::str::from_utf8(&current[..len]).map_err(|_| ParseError::InvalidUtf8)
    }
}

impl<'a> Parseable<'a> for alloc::string::String {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        <&str>::parse(parser).map(alloc::borrow::ToOwned::to_owned)
    }
}

impl<'a> Parseable<'a> for i64 {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        let current = parser.state;
        let mut len = 0;

        if parser.peek_raw() == Some('-') {
            len += 1;
            parser.next_raw().unwrap();
        }

        match parser.peek_raw() {
            Some(c) if c.is_ascii_digit() => {
                len += 1;
                parser.next_raw().unwrap();
            }
            Some(c) => return Err(ParseError::UnexpectedCharacter(c)),
            _ => return Err(ParseError::UnexpectedEof),
        }

        while matches!(parser.peek_raw(), Some(c) if c.is_ascii_digit()) {
            len += 1;
            parser.next_raw().unwrap();
        }

        // FIXME: this is a valid place to do `from_utf8_unchecked` but does it
        // really matter?
        let s = core::str::from_utf8(&current[..len]).map_err(|_| ParseError::InvalidUtf8)?;

        // At this point, the only way parsing fails is if the integer is too
        // large for an `i64`
        s.parse().map_err(|_| ParseError::IntegerValueTooLarge)
    }
}

pub(super) struct RequiredWhitespace;
impl<'a> Parseable<'a> for RequiredWhitespace {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        let next = parser.next()?;
        if !next.is_ascii_whitespace() {
            return Err(ParseError::UnexpectedCharacter(next));
        }

        while let Some(c) = parser.peek() {
            if c.is_ascii_whitespace() {
                parser.next()?;
            } else {
                break;
            }
        }

        Ok(Self)
    }
}

impl<'a, T: Parseable<'a>> Parseable<'a> for Option<T> {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        Ok(parser.parse_or_rewind())
    }
}

impl<'a> Parseable<'a> for bool {
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        match parser.next()? {
            't' => {
                parser.eat('r')?;
                parser.eat('u')?;
                parser.eat('e')?;
                Ok(true)
            }
            'f' => {
                parser.eat('a')?;
                parser.eat('l')?;
                parser.eat('s')?;
                parser.eat('e')?;
                Ok(false)
            }
            c => Err(ParseError::UnexpectedCharacter(c)),
        }
    }
}

#[derive(Debug)]
pub(super) struct RepeatUntilNoTrail<T, U> {
    pub(super) values: alloc::vec::Vec<T>,
    _trail: PhantomData<U>,
}

impl<'a, T: Parseable<'a>, U> Parseable<'a> for RepeatUntilNoTrail<T, U>
where
    Option<U>: Parseable<'a>,
{
    #[inline]
    fn parse(parser: &mut Parser<'a>) -> Result<Self, ParseError> {
        let mut values = alloc::vec::Vec::new();
        while let Some((value, trail)) = parser.parse_or_rewind::<(T, Option<U>)>() {
            values.push(value);

            if trail.is_none() {
                break;
            }
        }

        Ok(Self { values, _trail: PhantomData })
    }
}
