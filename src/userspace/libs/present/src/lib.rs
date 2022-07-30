// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(const_btree_new, thread_local)]

pub mod interrupt;
pub mod ipc;
pub mod join;
pub mod reactor;
pub mod sync;
pub mod waker;

extern crate sync as sync_prims;

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use reactor::Reactor;
use sync_prims::{Lazy, };
use std::{collections::BTreeMap, sync::SyncRefCell};

pub struct Task {
    task_id: u64,
    future: Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>,
    waker: waker::ArcWaker,
}

pub(crate) static GLOBAL_EXECUTOR: SyncRefCell<Lazy<PresentExecutor>> =
    SyncRefCell::new(Lazy::new(|| PresentExecutor { next_task_id: 0, ready_tasks: VecDeque::new(), waiting_tasks: BTreeMap::new() }));

pub struct Present {}

impl Present {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(&self) {
        librust::syscalls::task::enable_notifications();
        loop {
            let mut executor = GLOBAL_EXECUTOR.borrow_mut();
            if executor.ready_tasks.is_empty() && executor.waiting_tasks.is_empty() {
                return;
            } else if executor.ready_tasks.is_empty() {
                drop(executor);
                Reactor::wait();
                continue;
            }

            let mut next = executor.ready_tasks.pop_front().unwrap();
            drop(executor);

            if next.future.as_mut().poll(&mut Context::from_waker(&next.waker.clone().into())).is_pending() {
                GLOBAL_EXECUTOR.borrow_mut().push_blocked(next);
            }
        }
    }

    fn run_with<F: Future>(&self, f: F) -> F::Output {
        let mut value = None;

        // SAFETY: We're single threaded and block the current thread until the
        // future finishes executing
        let waiting_on = unsafe {
            GLOBAL_EXECUTOR.borrow_mut().push_unchecked(async {
                value = Some(f.await);
            })
        };

        loop {
            let mut executor = GLOBAL_EXECUTOR.borrow_mut();
            if executor.ready_tasks.is_empty() {
                drop(executor);
                Reactor::wait();
                continue;
            }

            let mut next = executor.pop().unwrap();
            drop(executor);

            match next.future.as_mut().poll(&mut Context::from_waker(&next.waker.clone().into())) {
                Poll::Pending => GLOBAL_EXECUTOR.borrow_mut().push_blocked(next),
                Poll::Ready(_) if next.task_id == waiting_on => return value.unwrap(),
                _ => {}
            }
        }
    }

    pub fn block_on<F>(&self, f: F) -> F::Output
    where
        F: Future,
    {
        self.run_with(f)
    }
}

pub fn spawn<F>(f: F) -> join::JoinHandle<F::Output>
where
    F: Future + Send + Sync + 'static,
    F::Output: Send + 'static
{
    let (tx, rx) = sync::oneshot::oneshot();
    GLOBAL_EXECUTOR.borrow_mut().push_new(async move {
        tx.send(f.await)
    });

    join::JoinHandle::new(rx)
}

pub struct PresentExecutor {
    next_task_id: u64,
    ready_tasks: VecDeque<Task>,
    waiting_tasks: BTreeMap<u64, Task>,
}

impl PresentExecutor {
    pub(crate) fn push_new<F: Future<Output=()> + Send + Sync + 'static>(&mut self, f: F) -> u64 {
        unsafe { self.push_unchecked(f) }
    }

    pub(crate) unsafe fn push_unchecked<F: Future<Output=()>>(&mut self, f: F) -> u64 {
        let task_id = self.next_task_id;
        self.ready_tasks.push_back(Task {
            task_id,
            future: core::mem::transmute(Box::pin(f) as Pin<Box<dyn Future<Output=()>>>),
            waker: waker::ArcWaker::new(waker::Waker { task_id }),
        });
        self.next_task_id += 1;
        task_id
    }

    pub(crate) fn pop(&mut self) -> Option<Task> {
        self.ready_tasks.pop_front()
    }

    pub(crate) fn push_blocked(&mut self, task: Task) {
        if self.waiting_tasks.insert(task.task_id, task).is_some() {
            panic!("double-waiting on the same task?");
        }
    }

    pub(crate) fn awaken(&mut self, id: u64) {
        if let Some(task) = self.waiting_tasks.remove(&id) {
            self.ready_tasks.push_back(task);
        }
    }
}

#[macro_export]
macro_rules! pin {
    ($i:ident) => {
        let mut $i = $i;
        let mut $i = unsafe { core::pin::Pin::new_unchecked(&mut $i) };
    };
}

#[macro_export]
macro_rules! select {
    ($($p:pat = $e:expr => $b:block)+) => {{
        struct Select2<F1: core::future::Future, F2: core::future::Future>(F1, F2);
        struct BottomedOut;
        enum Output<T, U> {
            T(T),
            U(U),
        }

        impl<F1, F2> core::future::Future for Select2<F1, F2>
        where
            F1: core::future::Future,
            F2: core::future::Future,
        {
            type Output = Output<F1::Output, F2::Output>;
            fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                match core::future::Future::poll(unsafe { core::pin::Pin::map_unchecked_mut(self.as_mut(), |s| &mut s.0) }, cx) {
                    core::task::Poll::Ready(t) => core::task::Poll::Ready(Output::T(t)),
                    core::task::Poll::Pending => match core::future::Future::poll(unsafe { core::pin::Pin::map_unchecked_mut(self, |s| &mut s.1) }, cx) {
                        core::task::Poll::Ready(u) => core::task::Poll::Ready(Output::U(u)),
                        core::task::Poll::Pending => core::task::Poll::Pending,
                    }
                }
            }
        }

        let mut select = $crate::select!(@genselect2 $($p = $e => $b)+);
        $crate::pin!(select);
        $crate::select!(@genselect2match select.await, $($p = $e => $b)+);
    }};

    (@genselect2 $p1:pat = $e1:expr => $b1:block $($p:pat = $e:expr => $b:block)+) => {
        Select2($e1, $crate::select!(@genselect2 $($p = $e => $b)+))
    };

    (@genselect2 $p:pat = $e:expr => $b:block) => {
        Select2($e, core::future::pending())
    };

    (@genselect2match $match_on:expr, $p1:pat = $e1:expr => $b1:block $($p:pat = $e:expr => $b:block)+) => {
        match $match_on {
            Output::T($p1) => $b1
            Output::U(u) => $crate::select!(@genselect2match u, $($p = $e => $b)+),
        }
    };

    (@genselect2match $match_on:expr, $p:pat = $e:expr => $b:block) => {
        match $match_on {
            Output::T($p) => $b,
            Output::U(BottomedOut) => {}
        }
    };
}

#[macro_export]
macro_rules! main {
    (async fn main() $b:block) => {
        fn main() {
            let present = $crate::Present::new();
            present.block_on(async { $b });
        }
    };
    ($b:block) => {
        fn main() {
            let present = $crate::Present::new();
            present.block_on(async { $b });
        }
    };
}