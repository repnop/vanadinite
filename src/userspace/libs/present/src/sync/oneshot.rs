// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

// TODO: more efficient way to pass things?

use crate::executor::reactor::{BlockType, EVENT_REGISTRY};
use core::{future::Future, pin::Pin};
use std::{
    sync::{SyncRc, SyncRefCell},
    task::{Context, Poll},
};

pub struct OneshotTx<T: Send + 'static> {
    inner: SyncRc<SyncRefCell<Option<T>>>,
    id: u64,
}

impl<T: Send + 'static> OneshotTx<T> {
    pub fn send(self, value: T) {
        *self.inner.borrow_mut() = Some(value);
        if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::AsyncChannel(self.id)) {
            waker.wake();
        }
    }
}

impl<T: Send + 'static> core::fmt::Debug for OneshotTx<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OneshotTx").finish_non_exhaustive()
    }
}

pub struct OneshotRx<T: Send + 'static> {
    inner: SyncRc<SyncRefCell<Option<T>>>,
    id: u64,
}

impl<T: Send + 'static> OneshotRx<T> {
    pub async fn recv(self) -> T {
        OneshotRxRecv(self).await
    }
}

impl<T: Send + 'static> core::fmt::Debug for OneshotRx<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OneshotRx").finish_non_exhaustive()
    }
}

struct OneshotRxRecv<T: Send + 'static>(OneshotRx<T>);

impl<T: Send + 'static> Future for OneshotRxRecv<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.0.inner.borrow_mut();
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
    let inner = SyncRc::new(SyncRefCell::new(None));

    (OneshotTx { inner: SyncRc::clone(&inner), id }, OneshotRx { inner, id })
}
