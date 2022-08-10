// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alloc::rc::Rc;

use crate::{
    stream::{ErrorCollectStream, Stream},
    Error, Parser,
};

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
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        self.parser.parse(stream)
    }
}

pub fn try_adapter<I, O, E, P>(parser: P) -> TryAdapter<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    TryAdapter { parser }
}

#[derive(Debug)]
pub struct TryAdapter<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    parser: P,
}

impl<I, O, E, P> TryAdapter<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I>,
{
    pub fn new(parser: P) -> Self {
        Self { parser }
    }
}

impl<I, O, E, P> Clone for TryAdapter<I, O, E, P>
where
    E: Error,
    P: Parser<Error = E, Output = O, Input = I> + Clone,
{
    fn clone(&self) -> Self {
        Self { parser: self.parser.clone() }
    }
}

impl<I, O, E, P> Parser for TryAdapter<I, O, E, P>
where
    I: core::fmt::Debug + Clone,
    O: core::fmt::Debug + Clone,
    E: Error + Clone,
    P: Parser<Error = E, Output = O, Input = I>,
{
    type Error = P::Error;
    type Output = O;
    type Input = Result<I, E>;

    #[inline]
    fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
    where
        S: Stream<Item = Self::Input>,
    {
        let mut error_collect = ErrorCollectStream { stream: stream.clone(), error_dump: None };
        match self.parser.parse(&mut error_collect) {
            Ok(output) => {
                core::mem::swap(stream, &mut error_collect.stream);
                Ok(output)
            }
            Err(e) => match error_collect.error_dump {
                Some(e) => Err(e),
                None => Err(e),
            },
        }
    }
}
