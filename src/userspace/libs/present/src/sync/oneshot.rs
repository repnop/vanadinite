// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

// TODO: more efficient way to pass things?

use core::{future::Future, pin::Pin};
use std::{sync::{ Arc}, task::{Context, Poll}};
use sync::{SpinMutex, Immediate};
use crate::reactor::{EVENT_REGISTRY, BlockType};

pub struct OneshotTx<T: Send + 'static> {
    inner: Arc<SpinMutex<Option<T>, Immediate>>,
    id: u64,
}

impl<T: Send + 'static> OneshotTx<T> {
    pub fn send(self, value: T) {
        *self.inner.lock() = Some(value);
        if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::AsyncChannel(self.id)) {
            waker.wake();
        }
    }
}

pub struct OneshotRx<T: Send + 'static> {
    inner: Arc<SpinMutex<Option<T>, Immediate>>,
    id: u64,
}

impl<T: Send + 'static> OneshotRx<T> {
    pub async fn recv(self) -> T {
        OneshotRxRecv(self).await
    }
}

struct OneshotRxRecv<T: Send + 'static>(OneshotRx<T>);

impl<T: Send + 'static> Future for OneshotRxRecv<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.0.inner.lock();
        match locked.take() {
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

pub fn oneshot<T: Send + 'static>() -> (OneshotTx<T>, OneshotRx<T>) {
    let id = super::CHANNEL_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    let inner = Arc::new(SpinMutex::new(None));

    (OneshotTx { inner: Arc::clone(&inner), id }, OneshotRx { inner, id })
}
