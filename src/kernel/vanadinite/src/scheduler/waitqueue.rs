// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{sync::SpinMutex, task::TaskState, utils::SameHartDeadlockDetection};
use alloc::collections::VecDeque;
use librust::task::Tid;

use super::{CURRENT_TASK, SCHEDULER};

#[derive(Debug)]
pub struct WaitQueue {
    queue: SpinMutex<VecDeque<Tid>, SameHartDeadlockDetection>,
}

impl WaitQueue {
    pub fn new() -> Self {
        Self { queue: SpinMutex::new(VecDeque::new()) }
    }

    #[track_caller]
    pub fn wait(&self) {
        self.queue.lock().push_back(CURRENT_TASK.borrow().tid);
        SCHEDULER.schedule(TaskState::Blocked);
    }

    #[track_caller]
    pub fn wake_one(&self) {
        if let Some(tid) = self.queue.lock().pop_front() {
            log::debug!("Waking task in waitqueue {:?}", tid);
            SCHEDULER.wake(tid);
        }
    }

    #[track_caller]
    pub fn wake_all(&self) {
        let mut queue = self.queue.lock();
        for tid in queue.drain(..) {
            log::debug!("Waking task in waitqueue {:?}", tid);
            SCHEDULER.wake(tid);
        }
    }
}
