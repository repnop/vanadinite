// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Error, Parser};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
};
use core::cell::RefCell;

pub fn recursive<E, I, O, P>(f: impl FnOnce(Recursive<E, I, O>) -> P) -> Recursive<E, I, O>
where
    E: Error,
    I: core::fmt::Debug,
    P: Parser<Error = E, Input = I, Output = O> + 'static,
{
    let recursive = Recursive::new();
    let parser = f(recursive.clone());
    recursive.set(parser);

    recursive
}

pub struct Recursive<E, I, O> {
    parser: RecursiveInner<E, I, O>,
}

impl<E, I, O> Recursive<E, I, O> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { parser: RecursiveInner::Owner(Rc::new(RefCell::new(None))) }
    }

    pub fn set<P>(&self, parser: P)
    where
        P: Parser<Error = E, Input = I, Output = O> + 'static,
    {
        match &self.parser {
            RecursiveInner::Owner(own) => *own.borrow_mut() = Some(Box::new(parser) as _),
            RecursiveInner::Borrower(borrowed) => {
                if let Some(own) = borrowed.upgrade() {
                    *own.borrow_mut() = Some(Box::new(parser) as _);
                }
            }
        }
    }
}

impl<E, I, O> Clone for Recursive<E, I, O> {
    fn clone(&self) -> Self {
        match &self.parser {
            RecursiveInner::Owner(owner) => Self { parser: RecursiveInner::Borrower(Rc::downgrade(owner)) },
            RecursiveInner::Borrower(borrowed) => Self { parser: RecursiveInner::Borrower(Weak::clone(borrowed)) },
        }
    }
}

enum RecursiveInner<E, I, O> {
    Owner(Rc<RefCell<Option<Box<dyn Parser<Error = E, Input = I, Output = O>>>>>),
    Borrower(Weak<RefCell<Option<Box<dyn Parser<Error = E, Input = I, Output = O>>>>>),
}

impl<E, I, O> Parser for Recursive<E, I, O>
where
    E: Error,
    I: core::fmt::Debug,
{
    type Error = E;
    type Input = I;
    type Output = O;

    fn parse(&self, stream: &mut crate::stream::Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
        match &self.parser {
            RecursiveInner::Owner(owner) => owner.borrow().as_ref().expect("no parser set!").parse(stream),
            RecursiveInner::Borrower(borrowed) => match Weak::upgrade(borrowed) {
                Some(parser) => parser.borrow().as_ref().expect("no parser set!").parse(stream),
                None => panic!("parser called after owner was dropped!"),
            },
        }
    }
}
