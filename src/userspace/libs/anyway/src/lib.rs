// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(thin_box)]
#![no_std]

extern crate alloc;

use alloc::{
    boxed::{Box, ThinBox},
    vec,
    vec::Vec,
};
use core::{any::Any, fmt::Display};

pub struct Error(Option<ThinBox<ErrorInner>>);

impl Error {
    pub fn new<E: Display + Sync + Send + 'static>(e: E) -> Self {
        Self(Some(ThinBox::new(ErrorInner { context: Vec::new(), error: Box::new(e) })))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = self.0.as_ref().unwrap();
        match f.alternate() {
            false => inner.error.fmt(f),
            true => {
                write!(f, "error: {}", inner.error)?;

                if !inner.context.is_empty() {
                    for (i, context) in inner.context.iter().enumerate() {
                        writeln!(f, "    #{} caused by: {}", i, context)?;
                    }
                }

                Ok(())
            }
        }
    }
}

struct ErrorInner {
    context: Vec<Box<dyn Display>>,
    error: Box<dyn Display>,
}

pub trait Context<T> {
    fn context<C: Display + Send + Sync + 'static>(self, ctx: C) -> Result<T, Error>;
    fn with_context<C: Display + Send + Sync + 'static>(self, f: impl FnOnce() -> C) -> Result<T, Error>;
}

impl<T, E: Display + Send + Sync + 'static> Context<T> for Result<T, E> {
    fn context<C: Display + Send + Sync + 'static>(self, ctx: C) -> Result<T, Error> {
        self.with_context(move || ctx)
    }

    fn with_context<C: Display + Send + Sync + 'static>(self, f: impl FnOnce() -> C) -> Result<T, Error> {
        self.map_err(|mut e| match upcast_mut(&mut e).downcast_mut::<Error>() {
            Some(error) => {
                let mut inner = error.0.take().unwrap();
                inner.context.push(Box::new(f()));
                Error(Some(inner))
            }
            None => Error(Some(ThinBox::new(ErrorInner { context: vec![Box::new(f())], error: Box::new(e) }))),
        })
    }
}

fn upcast_mut<T: 'static>(t: &mut T) -> &mut dyn Any {
    t
}
