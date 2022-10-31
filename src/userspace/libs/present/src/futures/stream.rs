// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod from_iter;
mod helpers;
mod map;
mod merge;
mod next;

use core::pin::Pin;
use std::task::{Context, Poll};

pub use from_iter::{from_iter, FromIter};

pub trait Stream {
    type Item;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>>;
}

pub trait IntoStream {
    type Item;
    type Stream: Stream<Item = Self::Item>;

    fn into_stream(self) -> Self::Stream;
}

impl<S: Stream> IntoStream for S {
    type Item = S::Item;
    type Stream = Self;

    fn into_stream(self) -> Self::Stream {
        self
    }
}

pub trait StreamExt: Stream {
    fn next(&mut self) -> next::Next<Self>
    where
        Self: Unpin,
    {
        next::Next { stream: self }
    }

    fn map<U, F>(self, f: F) -> map::Map<Self, U, F>
    where
        Self: Sized,
        F: Fn(Self::Item) -> U,
    {
        map::Map { stream: self, map: f }
    }

    fn merge<S>(self, other: S) -> merge::Merge<Self, S>
    where
        Self: Sized,
        S: Stream<Item = Self::Item>,
    {
        merge::Merge { helper: helpers::AlternatingPollOrder::new(self, other) }
    }
}

impl<S: Stream> StreamExt for S {}
