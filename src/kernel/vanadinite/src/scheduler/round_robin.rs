// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;

use super::SchedulerPolicy;
use crate::task::{Task, TaskState};
use alloc::{collections::VecDeque, sync::Arc};
use librust::task::Tid;

pub struct RoundRobinPolicy {
    tasks: VecDeque<Arc<Task>>,
    idle_tid: Tid,
}

impl RoundRobinPolicy {
    pub fn new() -> Self {
        Self { tasks: VecDeque::new(), idle_tid: Tid::new(NonZeroUsize::new(usize::MAX).unwrap()) }
    }
}

impl SchedulerPolicy for RoundRobinPolicy {
    fn next(&mut self) -> Tid {
        match self.tasks.is_empty() {
            true => self.idle_tid,
            false => {
                for _ in 0..self.tasks.len() {
                    self.tasks.rotate_left(1);

                    if !matches!(self.tasks.front().unwrap().mutable_state.lock().state, TaskState::Ready) {
                        continue;
                    }

                    return self.tasks.front().unwrap().tid;
                }

                self.idle_tid
            }
        }
    }

    fn task_enqueued(&mut self, tid: Arc<Task>, _metadata: super::TaskMetadata) {
        self.tasks.push_back(tid)
    }

    fn task_dequeued(&mut self, tid: Tid) {
        match self.tasks.iter().position(|t| t.tid == tid) {
            Some(index) => drop(self.tasks.remove(index)),
            None => unreachable!("Asked to remove TID that doesn't exist in policy: {:?}", tid),
        }
    }

    fn task_priority_changed(&mut self, _: Tid, _: u16) {}
    fn task_preempted(&mut self, _: Tid) {}

    fn idle_task(&mut self, tid: Tid) {
        self.idle_tid = tid;
    }
}
