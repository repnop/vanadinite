// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::pin::Pin;
use std::task::{Context, Poll};

use super::Stream;

#[derive(Debug)]
#[must_use = "`Future`s must be awaited or polled to do anything"]
pub struct Map<S: Stream, U, F: Fn(S::Item) -> U> {
    pub(super) stream: S,
    pub(super) map: F,
}

impl<S: Stream, U, F: Fn(S::Item) -> U> Stream for Map<S, U, F> {
    type Item = U;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        let (stream, f) = unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.stream), &this.map)
        };

        match stream.poll_next(context) {
            Poll::Ready(Some(t)) => Poll::Ready(Some(f(t))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
