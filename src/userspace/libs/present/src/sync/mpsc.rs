// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{executor::reactor::{BlockType, EVENT_REGISTRY}, futures::stream::{IntoStream, Stream}};
use core::{future::Future, pin::Pin};
use std::{sync::{SyncRc, SyncRefCell}, task::{Context, Poll}};

#[derive(Debug)]
pub struct Sender<T: Send + 'static> {
    inner: SyncRc<SyncRefCell<VecDeque<T>>>,
    id: u64,
}

impl<T: Send + 'static> Sender<T> {
    pub fn send(&self, value: T) {
        self.inner.borrow_mut().push_back(value);
        if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::AsyncChannel(self.id)) {
            waker.wake();
        }
    }
}

impl<T: Send + 'static> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: SyncRc::clone(&self.inner),
            id: self.id
        }
    }
}

#[derive(Debug)]
pub struct Receiver<T: Send + 'static> {
    inner: SyncRc<SyncRefCell<VecDeque<T>>>,
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
        let mut locked = self.0.inner.borrow_mut();
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

impl<T: Send + 'static> IntoStream for Receiver<T> {
    type Item = T;
    type Stream = ReceiverStream<T>;

    fn into_stream(self) -> Self::Stream {
        ReceiverStream { receiver: self }
    }
}

pub struct ReceiverStream<T: Send + 'static> {
    receiver: Receiver<T>,
}

impl<T: Send + 'static> Stream for ReceiverStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut locked = self.receiver.inner.borrow_mut();
        match locked.pop_front() {
            Some(t) => Poll::Ready(Some(t)),
            None => {
                // Note: its important to keep the lock held while we register
                // to wake here, so to avoid a TOCTOU race condition and losing
                // the wake event
                EVENT_REGISTRY.register(BlockType::AsyncChannel(self.receiver.id), cx.waker().clone());
                drop(locked);
                Poll::Pending
            }
        }
    }
}

pub fn unbounded<T: Send + 'static>() -> (Sender<T>, Receiver<T>) {
    let id = super::CHANNEL_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    let inner = SyncRc::new(SyncRefCell::new(VecDeque::new()));

    (Sender { inner: SyncRc::clone(&inner), id }, Receiver { inner, id })
}
