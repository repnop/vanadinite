// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Error, Parser, Span};

pub trait Stream: Clone {
    type Item: core::fmt::Debug + Clone;

    fn next(&mut self) -> Option<(Self::Item, Span)>;
    fn peek(&mut self) -> Option<(Self::Item, Span)>;
}

// impl<I, Item> Stream for I
// where
//     I: Iterator<Item = (Item, Span)>,
// {
//     type Item<'a> = Item;
//     fn next<'a>(&mut self) -> Option<(Self::Item<'a>, Span)> {
//         Iterator::next(self)
//     }
// }

#[derive(Debug, Clone)]
pub struct CharStream<'a> {
    iter: core::iter::Peekable<core::str::CharIndices<'a>>,
}

impl<'a> CharStream<'a> {
    pub fn new(s: &'a str) -> CharStream {
        Self { iter: s.char_indices().peekable() }
    }
}

impl Stream for CharStream<'_> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<(Self::Item, Span)> {
        let (index, next) = self.iter.next()?;
        Some((next, Span { start: index, end: next.len_utf8() }))
    }

    #[inline]
    fn peek<'a>(&mut self) -> Option<(Self::Item, Span)> {
        let (index, next) = self.iter.peek()?;
        Some((*next, Span { start: *index, end: next.len_utf8() }))
    }
}

pub struct ParserOutputStream<E, I, O, S, P>
where
    E: Error + Clone,
    I: core::fmt::Debug + Clone,
    O: core::fmt::Debug,
    S: Stream<Item = I>,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: P,
    stream: S,
    lookahead: Option<(Result<O, E>, Span)>,
}

impl<E, I, O, S, P> ParserOutputStream<E, I, O, S, P>
where
    E: Error + Clone,
    I: core::fmt::Debug + Clone,
    O: core::fmt::Debug + Clone,
    S: Stream<Item = I>,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    pub fn new(parser: P, stream: S) -> Self {
        Self { parser, stream, lookahead: None }
    }
}

impl<E, I, O, S, P> Clone for ParserOutputStream<E, I, O, S, P>
where
    E: Error + Clone,
    I: core::fmt::Debug + Clone,
    O: core::fmt::Debug + Clone,
    S: Stream<Item = I>,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { parser: self.parser.clone(), stream: self.stream.clone(), lookahead: self.lookahead.clone() }
    }
}

impl<E, I, O, S, P> Stream for ParserOutputStream<E, I, O, S, P>
where
    E: Error + Clone,
    I: core::fmt::Debug + Clone,
    O: core::fmt::Debug + Clone,
    S: Stream<Item = I>,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    type Item = Result<O, E>;

    fn next(&mut self) -> Option<(Self::Item, Span)> {
        if let Some(next) = self.lookahead.take() {
            return Some(next);
        }

        let (_, span) = self.stream.peek()?;
        Some((self.parser.parse(&mut self.stream), span))
    }

    fn peek(&mut self) -> Option<(Self::Item, Span)> {
        if let Some(next) = self.lookahead.as_ref() {
            return Some(next.clone());
        }

        let (_, span) = self.stream.peek()?;
        let next = (self.parser.parse(&mut self.stream.clone()), span);
        self.lookahead = Some(next.clone());

        Some(next)
    }
}

pub struct ErrorCollectStream<E, I, S>
where
    E: Error + Clone,
    I: core::fmt::Debug,
    S: Stream<Item = Result<I, E>>,
{
    pub(crate) stream: S,
    pub(crate) error_dump: Option<E>,
}

impl<E, I, S> Clone for ErrorCollectStream<E, I, S>
where
    E: Error + Clone,
    I: core::fmt::Debug,
    S: Stream<Item = Result<I, E>>,
{
    fn clone(&self) -> Self {
        Self { stream: self.stream.clone(), error_dump: self.error_dump.clone() }
    }
}

impl<E, I, S> Stream for ErrorCollectStream<E, I, S>
where
    E: Error + Clone,
    I: core::fmt::Debug + Clone,
    S: Stream<Item = Result<I, E>>,
{
    type Item = I;

    fn next(&mut self) -> Option<(Self::Item, crate::Span)> {
        let (next, span) = self.stream.next()?;

        match next {
            Ok(item) => Some((item, span)),
            Err(e) => {
                self.error_dump = Some(e);
                None
            }
        }
    }

    fn peek(&mut self) -> Option<(Self::Item, crate::Span)> {
        let (next, span) = self.stream.peek()?;

        match next {
            Ok(item) => Some((item, span)),
            Err(e) => {
                self.error_dump = Some(e);
                None
            }
        }
    }
}
