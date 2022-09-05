// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::Span;

pub struct Stream<'a, T> {
    source: alloc::boxed::Box<dyn StreamSource<Item = T> + 'a>,
    pub(crate) buffer: alloc::collections::VecDeque<(T, Span)>,
    pub(crate) mode: StreamMode,
}

impl<'a, T> Stream<'a, T>
where
    T: Clone + core::fmt::Debug,
{
    #[inline]
    pub fn new<S>(source: S) -> Self
    where
        S: StreamSource<Item = T> + 'a,
        T: 'a,
    {
        Self {
            source: alloc::boxed::Box::new(source),
            buffer: alloc::collections::VecDeque::new(),
            mode: StreamMode::Normal,
        }
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(T, Span)> {
        match &mut self.mode {
            // Try to get an already obtained element, otherwise grab the next one
            StreamMode::Transaction { current, .. } => match self.buffer.get(*current) {
                Some(value) => {
                    *current += 1;
                    Some(<(T, Span)>::clone(value))
                }
                None => {
                    *current += 1;
                    self.buffer.push_back(self.source.next()?);
                    self.buffer.back().cloned()
                }
            },
            StreamMode::Normal => {
                if let Some(next) = self.buffer.pop_front() {
                    return Some(next);
                }

                self.source.next()
            }
        }
    }

    #[inline]
    pub fn peek(&mut self) -> Option<(&T, Span)> {
        match &mut self.mode {
            StreamMode::Transaction { current, .. } => {
                if self.buffer.get(*current).is_some() {
                    return self.buffer.get(*current).map(|(t, span)| (t, *span));
                }

                self.buffer.push_back(self.source.next()?);
                self.buffer.back().map(|(t, span)| (t, *span))
            }
            StreamMode::Normal => {
                if self.buffer.front().is_some() {
                    return self.buffer.front().map(|(peek, span)| (peek, *span));
                }

                let next = self.next()?;
                self.buffer.push_front(next);
                self.buffer.front().map(|(next, span)| (next, *span))
            }
        }
    }

    pub(crate) fn try_parse<E, O>(&mut self, f: impl FnOnce(&mut Self) -> Result<O, E>) -> Result<O, E> {
        let mut current_transaction = None;
        match self.mode {
            // Create a new transaction
            StreamMode::Normal => {
                self.mode = StreamMode::Transaction { start: 0, current: 0 };
            }
            // Overlay a new transaction that will be reset if it fails
            StreamMode::Transaction { start, current } => {
                current_transaction = Some(start);
                self.mode = StreamMode::Transaction { start: current, current };
            }
        }

        match (f(self), current_transaction) {
            (res, Some(start)) => {
                match res {
                    // Progress further in the stream
                    Ok(_) => {
                        let StreamMode::Transaction { current, .. } = self.mode else { unreachable!() };
                        self.mode = StreamMode::Transaction { start: current, current };
                    }
                    // Rollback to the original starting position
                    Err(_) => self.mode = StreamMode::Transaction { start, current: start },
                }

                res
            }
            (res, None) => {
                if res.is_ok() {
                    let StreamMode::Transaction { current, .. } = self.mode else { unreachable!() };
                    // Commit to having gotten this far and free memory
                    // associated with already processed elements
                    self.buffer.drain(..current);
                }

                self.mode = StreamMode::Normal;
                res
            }
        }
    }

    pub(crate) fn in_try_mode(&self) -> bool {
        matches!(self.mode, StreamMode::Transaction { .. })
    }
}

impl<'a> Stream<'a, char> {
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &'a str) -> Stream<'a, char> {
        Stream::new(CharStream::new(s))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StreamMode {
    Normal,
    Transaction { start: usize, current: usize },
}

pub trait StreamSource {
    type Item;

    fn next(&mut self) -> Option<(Self::Item, Span)>;
}

impl<I, Item> StreamSource for I
where
    I: Iterator<Item = (Item, Span)>,
{
    type Item = Item;

    fn next(&mut self) -> Option<(Self::Item, Span)> {
        Iterator::next(self)
    }
}

#[derive(Debug, Clone)]
pub struct CharStream<'a> {
    iter: core::str::CharIndices<'a>,
}

impl<'a> CharStream<'a> {
    pub fn new(s: &'a str) -> CharStream {
        Self { iter: s.char_indices() }
    }
}

impl StreamSource for CharStream<'_> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<(Self::Item, Span)> {
        let (index, next) = self.iter.next()?;
        Some((next, Span { start: index, end: index + next.len_utf8() }))
    }
}

// pub struct ParserOutputStream<E, I, O, P>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug + Clone,
//     O: core::fmt::Debug,
//     P: Parser<Error = E, Output = O, Input = I>,
// {
//     parser: P,
//     stream: Stream<I>,
//     lookahead: Option<(Result<O, E>, Span)>,
// }

// impl<E, I, O, P> ParserOutputStream<E, I, O, P>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug + Clone,
//     O: core::fmt::Debug + Clone,
//     P: Parser<Error = E, Output = O, Input = I> + Clone,
// {
//     pub fn new(parser: P, stream: Stream<I>) -> Self {
//         Self { parser, stream, lookahead: None }
//     }
// }

// impl<E, I, O, P> Clone for ParserOutputStream<E, I, O, P>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug + Clone,
//     O: core::fmt::Debug + Clone,
//     P: Parser<Error = E, Output = O, Input = I> + Clone,
// {
//     fn clone(&self) -> Self {
//         Self { parser: self.parser.clone(), stream: self.stream.clone(), lookahead: self.lookahead.clone() }
//     }
// }

// impl<E, I, O, P> StreamSource for ParserOutputStream<E, I, O, P>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug + Clone,
//     O: core::fmt::Debug + Clone,
//     P: Parser<Error = E, Output = O, Input = I> + Clone,
// {
//     type Item = Result<O, E>;

//     fn next(&mut self) -> Option<(Self::Item, Span)> {
//         if let Some(next) = self.lookahead.take() {
//             return Some(next);
//         }

//         let (_, span) = self.stream.peek()?;
//         Some((self.parser.parse(&mut self.stream), span))
//     }
// }

// pub struct ErrorCollectStream<E, I>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug,
// {
//     stream: Stream<Result<I, E>>,
//     error_dump: Option<E>,
//     lookahead: Option<(I, Span)>,
// }

// impl<E, I> ErrorCollectStream<E, I>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug,
// {
//     pub fn new(stream: Stream<Result<I, E>>) -> Self {
//         Self { stream, error_dump: None, lookahead: None }
//     }

//     pub fn error(&mut self) -> Option<E> {
//         self.error_dump.take()
//     }

//     pub fn is_error(&self) -> bool {
//         self.error_dump.is_some()
//     }

//     pub fn into_parts(self) -> (Stream<Result<I, E>>, Option<E>) {
//         (self.stream, self.error_dump)
//     }
// }

// impl<E, I> Clone for ErrorCollectStream<E, I>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug,
// {
//     fn clone(&self) -> Self {
//         Self { stream: self.stream.clone(), error_dump: self.error_dump.clone(), lookahead: None }
//     }
// }

// impl<E, I> StreamSource for ErrorCollectStream<E, I>
// where
//     E: Error + Clone,
//     I: core::fmt::Debug + Clone,
// {
//     type Item = I;

//     fn next(&mut self) -> Option<(Self::Item, Span)> {
//         match (&mut self.error_dump, &mut self.lookahead) {
//             (None, Some(_)) => self.lookahead.take(),
//             (Some(_), _) => None,
//             (None, None) => match self.stream.next() {
//                 Some((Ok(next), span)) => Some((next, span)),
//                 Some((Err(e), _)) => {
//                     self.error_dump = Some(e);
//                     None
//                 }
//                 None => None,
//             },
//         }
//     }
// }
