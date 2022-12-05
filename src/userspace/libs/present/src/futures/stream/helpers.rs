// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{future::Future, pin::Pin};
use std::task::{Context, Poll};

use super::Stream;

#[derive(Debug)]
#[must_use = "`Future`s must be awaited or polled to do anything"]
pub struct AlternatingPollOrder<T, U> {
    pub(super) t: T,
    pub(super) u: U,
    pub(super) order: bool,
}

impl<T, U> AlternatingPollOrder<T, U> {
    pub(crate) fn new(t: T, u: U) -> Self {
        Self { t, u, order: true }
    }

    pub(crate) fn split(self) -> (T, U) {
        (self.t, self.u)
    }
}

impl<O, T: Future<Output = O>, U: Future<Output = O>> Future for AlternatingPollOrder<T, U> {
    type Output = O;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (t, u, order) = unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.t), Pin::new_unchecked(&mut this.u), &mut this.order)
        };
        let current_order = *order;
        *order = !*order;

        match current_order {
            true => match t.poll(cx) {
                Poll::Pending => u.poll(cx),
                Poll::Ready(out) => Poll::Ready(out),
            },
            false => match u.poll(cx) {
                Poll::Pending => t.poll(cx),
                Poll::Ready(out) => Poll::Ready(out),
            },
        }
    }
}

impl<O, T: Stream<Item = O>, U: Stream<Item = O>> Stream for AlternatingPollOrder<T, U> {
    type Item = O;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (t, u, order) = unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.t), Pin::new_unchecked(&mut this.u), &mut this.order)
        };
        let current_order = *order;
        *order = !*order;

        match current_order {
            true => match t.poll_next(context) {
                Poll::Pending => u.poll_next(context),
                Poll::Ready(out) => Poll::Ready(out),
            },
            false => match u.poll_next(context) {
                Poll::Pending => t.poll_next(context),
                Poll::Ready(out) => Poll::Ready(out),
            },
        }
    }
}
