// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::reactor::{BlockType, EVENT_REGISTRY};
use core::{future::Future, pin::Pin};
use std::task::{Context, Poll};

#[derive(Debug)]
pub struct Interrupt(usize);

impl Interrupt {
    pub fn new(n: usize) -> Self {
        EVENT_REGISTRY.register_interest(BlockType::Interrupt(n));
        Self(n)
    }

    pub async fn wait(&self) {
        InterruptWait(self.0).await;
    }
}

struct InterruptWait(usize);
impl Future for InterruptWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match EVENT_REGISTRY.consume_interest_event(BlockType::Interrupt(self.0)) {
            true => Poll::Ready(()),
            false => {
                EVENT_REGISTRY.register(BlockType::Interrupt(self.0), cx.waker().clone());
                Poll::Pending
            }
        }
    }
}
