// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    executor::reactor::{BlockType, EVENT_REGISTRY},
    futures::stream::Stream,
};
use core::{future::Future, pin::Pin};
use std::task::{Context, Poll};

#[derive(Debug, Clone, Copy)]
pub struct Interrupt(usize);

impl Interrupt {
    pub fn new(n: usize) -> Self {
        EVENT_REGISTRY.register_interest(BlockType::Interrupt(n));
        Self(n)
    }
}

impl Future for Interrupt {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match EVENT_REGISTRY.consume_interest_event(BlockType::Interrupt(self.0)) {
            true => Poll::Ready(self.0),
            false => {
                EVENT_REGISTRY.register(BlockType::Interrupt(self.0), cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl Stream for Interrupt {
    type Item = usize;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        match EVENT_REGISTRY.consume_interest_event(BlockType::Interrupt(self.0)) {
            true => Poll::Ready(Some(self.0)),
            false => {
                EVENT_REGISTRY.register(BlockType::Interrupt(self.0), context.waker().clone());
                Poll::Pending
            }
        }
    }
}
