// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::Span;
use alloc::format;

pub trait Error: Sized + core::fmt::Debug {
    fn custom<E: core::fmt::Display>(error: E, span: Option<Span>) -> Self;
    fn expected_one_of<V: core::fmt::Debug, S: AsRef<[V]>>(found: V, values: S, span: Option<Span>) -> Self {
        use core::fmt::Write;

        let mut s = alloc::string::String::from("expected one of ");

        for (i, v) in values.as_ref().iter().enumerate() {
            match i {
                0 => write!(&mut s, "`{:?}`", v).unwrap(),
                _ => write!(&mut s, ", `{:?}`", v).unwrap(),
            }
        }

        match span {
            Some(span) => write!(&mut s, " @ {}, found `{:?}`", span, found).unwrap(),
            None => write!(&mut s, ", found `{:?}`", found).unwrap(),
        }

        Self::custom(s, span)
    }

    fn unexpected_end_of_input() -> Self {
        Self::custom("unexpected end of input", None)
    }

    fn unexpected_value<V: core::fmt::Debug>(value: V, span: Option<Span>) -> Self {
        Self::custom(format!("unexpected value `{:?}`", value), span)
    }

    #[doc(hidden)]
    fn hopefully_cheap() -> Self {
        Self::custom("", None)
    }
}

impl Error for () {
    fn custom<E: core::fmt::Display>(_: E, _: Option<Span>) -> Self {}
}

impl Error for alloc::string::String {
    fn custom<E: core::fmt::Display>(error: E, span: Option<Span>) -> Self {
        use core::fmt::Write;

        let mut s = alloc::string::String::new();
        match span {
            Some(span) => write!(&mut s, "{} @ {}", error, span).unwrap(),
            None => write!(&mut s, "{}", error).unwrap(),
        }

        s
    }
}
