// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    sync::{mutex::SpinMutexGuard, DeadlockDetection, SpinMutex},
    task::{Task, TaskState},
    utils::SameHartDeadlockDetection,
};
use alloc::{collections::VecDeque, sync::Arc};

use super::{CURRENT_TASK, SCHEDULER};

#[derive(Debug, Clone)]
pub struct WaitQueue {
    // FIXME: this probably doesn't need the full `Arc`??
    queue: Arc<SpinMutex<VecDeque<Arc<Task>>, SameHartDeadlockDetection>>,
}

impl WaitQueue {
    pub fn new() -> Self {
        Self { queue: Arc::new(SpinMutex::new(VecDeque::new(), SameHartDeadlockDetection::new())) }
    }

    #[track_caller]
    pub fn wait<T: Send, D: DeadlockDetection>(&self, resource: &mut SpinMutexGuard<'_, T, D>) {
        let mut lock = self.queue.lock();
        let task = CURRENT_TASK.get();
        log::debug!("Placing self in waitqueue: [{:?}] {}", task.tid, task.name);
        lock.push_back(Arc::clone(&task));
        task.mutable_state.lock().state = TaskState::Blocked;
        unsafe { SpinMutexGuard::unlock(resource) };
        drop(lock);

        SCHEDULER.schedule();
        SpinMutexGuard::lock(resource);
    }

    #[track_caller]
    pub fn wake_one(&self) {
        if let Some(task) = self.queue.lock().pop_front() {
            log::debug!("Waking task in waitqueue: [{:?}] {}", task.tid, task.name);
            task.mutable_state.lock().state = TaskState::Ready;
        }
    }

    #[track_caller]
    pub fn wake_all(&self) {
        let mut queue = self.queue.lock();
        for task in queue.drain(..) {
            log::debug!("Waking task in waitqueue: [{:?}] {}", task.tid, task.name);
            task.mutable_state.lock().state = TaskState::Ready;
        }
    }
}
