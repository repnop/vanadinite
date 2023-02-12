// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    sync::SpinMutex,
    task::{Task, TaskState},
    utils::SameHartDeadlockDetection,
};
use alloc::{collections::VecDeque, sync::Arc};

use super::{CURRENT_TASK, SCHEDULER};

#[derive(Debug)]
pub struct WaitQueue {
    queue: SpinMutex<VecDeque<Arc<Task>>, SameHartDeadlockDetection>,
}

impl WaitQueue {
    pub fn new() -> Self {
        Self { queue: SpinMutex::new(VecDeque::new()) }
    }

    #[track_caller]
    pub fn wait(&self, f: impl FnOnce()) {
        let mut lock = self.queue.lock();
        let task = CURRENT_TASK.get();
        log::debug!("Placing self in waitqueue: [{:?}] {}", task.tid, task.name);
        lock.push_back(Arc::clone(&task));
        task.mutable_state.lock().state = TaskState::Blocked;
        f();
        drop(lock);

        SCHEDULER.schedule();
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
