// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub(crate) mod reactor;

use self::reactor::Reactor;
use crate::{
    join::JoinHandle,
    sync::oneshot,
    waker::{ArcWaker, Waker},
};
use core::{future::Future, pin::Pin};
use std::{
    collections::BTreeMap,
    sync::SyncRefCell,
    task::{Context, Poll},
};

pub struct Task {
    task_id: u64,
    future: Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>,
    waker: ArcWaker,
}

pub(crate) static GLOBAL_EXECUTOR: SyncRefCell<PresentExecutor> =
    SyncRefCell::new(PresentExecutor { next_task_id: 0, ready_tasks: Vec::new(), waiting_tasks: BTreeMap::new() });

pub struct Present {}

impl Present {
    #[allow(clippy::new_without_default)]
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

            let mut next = executor.ready_tasks.remove(0);
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

pub fn spawn<F>(f: F) -> JoinHandle<F::Output>
where
    F: Future + Send + Sync + 'static,
    F::Output: Send + 'static,
{
    let (tx, rx) = oneshot::oneshot();
    GLOBAL_EXECUTOR.borrow_mut().push_new(async move { tx.send(f.await) });

    JoinHandle::new(rx)
}

pub struct PresentExecutor {
    next_task_id: u64,
    ready_tasks: Vec<Task>,
    waiting_tasks: BTreeMap<u64, Task>,
}

impl PresentExecutor {
    pub(crate) fn push_new<F: Future<Output = ()> + Send + Sync + 'static>(&mut self, f: F) -> u64 {
        unsafe { self.push_unchecked(f) }
    }

    pub(crate) unsafe fn push_unchecked<F: Future<Output = ()>>(&mut self, f: F) -> u64 {
        let task_id = self.next_task_id;
        self.ready_tasks.push(Task {
            task_id,
            future: core::mem::transmute(Box::pin(f) as Pin<Box<dyn Future<Output = ()>>>),
            waker: ArcWaker::new(Waker { task_id }),
        });
        self.next_task_id += 1;
        task_id
    }

    pub(crate) fn pop(&mut self) -> Option<Task> {
        match self.ready_tasks.is_empty() {
            false => Some(self.ready_tasks.remove(0)),
            true => None,
        }
    }

    pub(crate) fn push_blocked(&mut self, task: Task) {
        if self.waiting_tasks.insert(task.task_id, task).is_some() {
            panic!("double-waiting on the same task?");
        }
    }

    pub(crate) fn awaken(&mut self, id: u64) {
        if let Some(task) = self.waiting_tasks.remove(&id) {
            self.ready_tasks.push(task);
        }
    }
}
