// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::reactor::{BlockType, EVENT_REGISTRY};
use core::{future::Future, pin::Pin};
use std::{sync::Arc, task::{Context, Poll}};
use sync::SpinMutex;

#[derive(Debug, Clone)]
pub struct Sender<T: Send + 'static> {
    inner: Arc<SpinMutex<VecDeque<T>>>,
    id: u64,
}

impl<T: Send + 'static> Sender<T> {
    pub fn send(&self, value: T) {
        self.inner.lock().push_back(value);
        if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::AsyncChannel(self.id)) {
            waker.wake();
        }
    }
}

#[derive(Debug)]
pub struct Receiver<T: Send + 'static> {
    inner: Arc<SpinMutex<VecDeque<T>>>,
    id: u64,
}

impl<T: Send + 'static> Receiver<T> {
    pub async fn recv(&self) -> T {
        ReceiverRecv(self).await
    }
}

struct ReceiverRecv<'a, T: Send + 'static>(&'a Receiver<T>);

impl<T: Send + 'static> Future for ReceiverRecv<'_, T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.0.inner.lock();
        match locked.pop_front() {
            Some(t) => Poll::Ready(t),
            None => {
                // Note: its important to keep the lock held while we register
                // to wake here, so to avoid a TOCTOU race condition and losing
                // the wake event
                EVENT_REGISTRY.register(BlockType::AsyncChannel(self.0.id), cx.waker().clone());
                drop(locked);
                Poll::Pending
            }
        }
    }
}

pub fn unbounded<T: Send + 'static>() -> (Sender<T>, Receiver<T>) {
    let id = super::CHANNEL_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    let inner = Arc::new(SpinMutex::new(VecDeque::new()));

    (Sender { inner: Arc::clone(&inner), id }, Receiver { inner, id })
}
