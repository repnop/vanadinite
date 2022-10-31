// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::task::{Context, Poll};

use super::Stream;

#[derive(Debug)]
#[must_use = "`Future`s must be awaited or polled to do anything"]
pub struct FromIter<I: Iterator> {
    iterator: I,
}

// A `Pin<&mut I>` is never created
impl<I: Iterator> Unpin for FromIter<I> {}
impl<I: Iterator> Stream for FromIter<I> {
    type Item = I::Item;

    fn poll_next(self: core::pin::Pin<&mut Self>, _context: &mut Context) -> std::task::Poll<Option<Self::Item>> {
        Poll::Ready(self.get_mut().iterator.next())
    }
}

pub fn from_iter<I: IntoIterator>(iter: I) -> FromIter<I::IntoIter> {
    FromIter { iterator: iter.into_iter() }
}
