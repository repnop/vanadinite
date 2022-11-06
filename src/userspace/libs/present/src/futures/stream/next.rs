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
pub struct Next<'a, S: ?Sized> {
    pub(crate) stream: &'a mut S,
}

impl<S: Stream + ?Sized + Unpin> Unpin for Next<'_, S> {}
impl<'a, S> Future for Next<'a, S>
where
    S: Stream + ?Sized + Unpin,
{
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.stream).poll_next(cx)
    }
}
