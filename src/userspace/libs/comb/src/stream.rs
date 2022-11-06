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
    debug: Option<DebugState>,
    span: (bool, Option<Span>),
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
            debug: None,
            span: (false, None),
        }
    }

    #[inline]
    pub fn with_debug<S, W>(source: S, writer: W) -> Self
    where
        S: StreamSource<Item = T> + 'a,
        T: 'a,
        W: core::fmt::Write + 'static,
    {
        Self {
            source: alloc::boxed::Box::new(source),
            buffer: alloc::collections::VecDeque::new(),
            mode: StreamMode::Normal,
            debug: Some(DebugState { writer: alloc::boxed::Box::new(writer), try_depth: 0 }),
            span: (false, None),
        }
    }

    #[inline]
    #[track_caller]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(T, Span)> {
        let caller = core::panic::Location::caller();
        match &mut self.mode {
            // Try to get an already obtained element, otherwise grab the next one
            StreamMode::Transaction { current, .. } => match self.buffer.get(*current) {
                Some(value) => {
                    *current += 1;
                    let value = <(T, Span)>::clone(value);

                    self.debug_action(DebugAction::TransactionConsume { item: &value.0 }, Some(caller));
                    if self.span.0 {
                        match &mut self.span.1 {
                            Some(span) => span.end = value.1.end,
                            this @ None => *this = Some(value.1),
                        }
                    }

                    Some(value)
                }
                None => {
                    *current += 1;
                    self.buffer.push_back(self.source.next()?);

                    let value = self.buffer.back().cloned().unwrap();

                    self.debug_action(DebugAction::TransactionConsume { item: &value.0 }, Some(caller));
                    if self.span.0 {
                        match &mut self.span.1 {
                            Some(span) => span.end = value.1.end,
                            this @ None => *this = Some(value.1),
                        }
                    }

                    Some(value)
                }
            },
            StreamMode::Normal => {
                if let Some(next) = self.buffer.pop_front() {
                    self.debug_action(DebugAction::NormalConsume { item: &next.0 }, Some(caller));
                    if self.span.0 {
                        match &mut self.span.1 {
                            Some(span) => span.end = next.1.end,
                            this @ None => *this = Some(next.1),
                        }
                    }
                    return Some(next);
                }

                let value = self.source.next();
                if let Some(value) = &value {
                    self.debug_action(DebugAction::NormalConsume { item: &value.0 }, Some(caller));
                    if self.span.0 {
                        match &mut self.span.1 {
                            Some(span) => span.end = value.1.end,
                            this @ None => *this = Some(value.1),
                        }
                    }
                }

                value
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

    #[inline]
    pub fn begin_record_span(&mut self) {
        self.span.0 = true;
    }

    #[inline]
    pub fn end_record_span(&mut self) -> Option<Span> {
        self.span.0 = false;
        self.span.1.take()
    }

    #[inline]
    pub(crate) fn try_parse<E, O>(&mut self, f: impl FnOnce(&mut Self) -> Result<O, E>) -> Result<O, E> {
        let mut current_transaction = None;
        match self.mode {
            // Create a new transaction
            StreamMode::Normal => {
                self.mode = StreamMode::Transaction { start: 0, current: 0 };
            }
            // Overlay a new transaction that will be reset if it fails
            StreamMode::Transaction { start, current } => {
                current_transaction = Some(current);
                if let Some(debug) = &mut self.debug {
                    debug.try_depth += 1;
                }
                self.debug_action(DebugAction::NewTransaction { previous_start: start, new_start: current }, None);
                self.mode = StreamMode::Transaction { start: current, current };
            }
        }

        match (f(self), current_transaction) {
            (res, Some(prev_current)) => {
                let StreamMode::Transaction { current, .. } = self.mode else { unreachable!() };

                match res {
                    // Progress further in the stream
                    Ok(_) => self.mode = StreamMode::Transaction { start: current, current },
                    // Rollback to the original buffer position
                    Err(_) => self.mode = StreamMode::Transaction { start: prev_current, current: prev_current },
                }

                self.debug_action(DebugAction::TransactionEnd { ok: res.is_ok() }, None);

                if let Some(debug) = &mut self.debug {
                    debug.try_depth -= 1;
                }

                res
            }
            (res, None) => {
                self.debug_action(DebugAction::TransactionEnd { ok: res.is_ok() }, None);

                if res.is_ok() {
                    let StreamMode::Transaction { current, .. } = self.mode else { unreachable!() };
                    // Commit to having gotten this far and free memory
                    // associated with already processed elements
                    self.buffer.drain(..current);
                    self.debug_action(DebugAction::Commit, None);
                }

                self.mode = StreamMode::Normal;
                res
            }
        }
    }

    pub(crate) fn in_try_mode(&self) -> bool {
        matches!(self.mode, StreamMode::Transaction { .. })
    }

    fn debug_action(
        &mut self,
        debug_action: DebugAction<'_, T>,
        caller: Option<&'static core::panic::Location<'static>>,
    ) {
        if let Some(debug) = self.debug.as_mut() {
            match debug_action {
                DebugAction::Commit => writeln!(
                    &mut debug.writer,
                    "{}[transaction] commit: buffer={:?}",
                    "+".repeat(debug.try_depth),
                    self.buffer,
                ),
                DebugAction::NewTransaction { previous_start, new_start } => writeln!(
                    &mut debug.writer,
                    "{}[transaction] new: previous_start={}, new_start={}",
                    "+".repeat(debug.try_depth),
                    previous_start,
                    new_start,
                ),
                DebugAction::NormalConsume { item } => {
                    let caller = caller.unwrap();
                    writeln!(
                        &mut debug.writer,
                        "{}[normal] consume: item={item:?} buffer={:?} caller = {}:{}:{}",
                        "+".repeat(debug.try_depth),
                        self.buffer,
                        caller.file(),
                        caller.line(),
                        caller.column(),
                    )
                }
                DebugAction::TransactionConsume { item } => {
                    let caller = caller.unwrap();
                    writeln!(
                        &mut debug.writer,
                        "{}[transaction] consume: item={item:?} buffer={:?} caller = {}:{}:{}",
                        "+".repeat(debug.try_depth),
                        self.buffer,
                        caller.file(),
                        caller.line(),
                        caller.column(),
                    )
                }
                DebugAction::TransactionEnd { ok } => {
                    writeln!(&mut debug.writer, "{}[transaction] finish: ok={ok}", "+".repeat(debug.try_depth),)
                }
            }
            .unwrap();
        }
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

struct DebugState {
    writer: alloc::boxed::Box<dyn core::fmt::Write>,
    try_depth: usize,
}

enum DebugAction<'a, T: core::fmt::Debug> {
    NewTransaction { previous_start: usize, new_start: usize },
    Commit,
    TransactionConsume { item: &'a T },
    NormalConsume { item: &'a T },
    TransactionEnd { ok: bool },
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
