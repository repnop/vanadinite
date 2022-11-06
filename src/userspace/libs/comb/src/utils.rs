// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alloc::rc::Rc;

use crate::{stream::Stream, Error, Parser};

pub fn cheap_clone<I, O, E, P>(parser: P) -> CheapClone<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    CheapClone { parser: Rc::new(parser) }
}

#[derive(Debug)]
pub struct CheapClone<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: Rc<P>,
}

impl<I, O, E, P> Clone for CheapClone<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    fn clone(&self) -> Self {
        Self { parser: Rc::clone(&self.parser) }
    }
}

impl<I, O, E, P> Parser for CheapClone<I, O, E, P>
where
    I: core::fmt::Debug,
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    type Error = P::Error;
    type Output = P::Output;
    type Input = I;

    #[inline]
    fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        self.parser.parse(stream)
    }
}

pub fn todo<E, I, O>() -> TodoParser<E, I, O>
where
    E: Error,
    I: core::fmt::Debug,
{
    TodoParser::new()
}

#[derive(Debug, Clone, Copy)]
pub struct TodoParser<E, I, O>(
    core::marker::PhantomData<fn() -> E>,
    core::marker::PhantomData<fn() -> I>,
    core::marker::PhantomData<fn() -> O>,
);

impl<E, I, O> TodoParser<E, I, O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<E, I, O> Default for TodoParser<E, I, O> {
    fn default() -> Self {
        Self(core::marker::PhantomData, core::marker::PhantomData, core::marker::PhantomData)
    }
}

impl<E, I, O> Parser for TodoParser<E, I, O>
where
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Input = I;
    type Output = O;

    #[inline]
    #[track_caller]
    fn parse(&self, _: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        todo!("make a parser")
    }
}

// pub fn try_adapter<I, O, E, P>(parser: P) -> TryAdapter<I, O, E, P>
// where
//     E: Error,
//     P: Parser<Error = E, Output = O, Input = I>,
// {
//     TryAdapter { parser }
// }

// #[derive(Debug)]
// pub struct TryAdapter<I, O, E, P>
// where
//     E: Error,
//     P: Parser<Error = E, Output = O, Input = I>,
// {
//     parser: P,
// }

// impl<I, O, E, P> TryAdapter<I, O, E, P>
// where
//     E: Error,
//     P: Parser<Error = E, Output = O, Input = I>,
// {
//     pub fn new(parser: P) -> Self {
//         Self { parser }
//     }
// }

// impl<I, O, E, P> Clone for TryAdapter<I, O, E, P>
// where
//     E: Error,
//     P: Parser<Error = E, Output = O, Input = I> + Clone,
// {
//     fn clone(&self) -> Self {
//         Self { parser: self.parser.clone() }
//     }
// }

// impl<I, O, E, P> Parser for TryAdapter<I, O, E, P>
// where
//     I: core::fmt::Debug + Clone,
//     O: core::fmt::Debug + Clone,
//     E: Error + Clone,
//     P: Parser<Error = E, Output = O, Input = I>,
// {
//     type Error = P::Error;
//     type Output = O;
//     type Input = Result<I, E>;

//     #[inline]
//     fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
//         let mut error_collect = ErrorCollectStream::new(stream.clone());
//         match self.parser.parse(&mut error_collect) {
//             Ok(output) => {
//                 core::mem::swap(stream, &mut error_collect.into_parts().0);
//                 Ok(output)
//             }
//             Err(e) => match error_collect.error() {
//                 Some(e) => Err(e),
//                 None => Err(e),
//             },
//         }
//     }
// }
