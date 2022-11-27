// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;

use super::SchedulerPolicy;
use crate::task::TaskState;
use alloc::collections::VecDeque;
use librust::task::Tid;

pub struct RoundRobinPolicy {
    tids: VecDeque<(Tid, TaskState)>,
    idle_tid: Tid,
}

impl RoundRobinPolicy {
    pub fn new() -> Self {
        Self { tids: VecDeque::new(), idle_tid: Tid::new(NonZeroUsize::new(usize::MAX).unwrap()) }
    }
}

impl SchedulerPolicy for RoundRobinPolicy {
    fn next(&mut self) -> Tid {
        match self.tids.is_empty() {
            true => self.idle_tid,
            false => {
                for _ in 0..self.tids.len() {
                    self.tids.rotate_left(1);

                    if !matches!(self.tids.front().unwrap().1, TaskState::Ready) {
                        continue;
                    }

                    return self.tids.front().unwrap().0;
                }

                self.idle_tid
            }
        }
    }

    fn task_enqueued(&mut self, tid: Tid, metadata: super::TaskMetadata) {
        self.tids.push_back((tid, metadata.run_state))
    }

    fn task_dequeued(&mut self, tid: Tid) {
        match self.tids.iter().position(|t| t.0 == tid) {
            Some(index) => drop(self.tids.remove(index)),
            None => unreachable!("Asked to remove TID that doesn't exist in policy: {:?}", tid),
        }
    }

    fn task_priority_changed(&mut self, _: Tid, _: u16) {}
    fn task_preempted(&mut self, _: Tid) {}

    fn update_state(&mut self, tid: Tid, state: TaskState) {
        self.tids.iter_mut().find(|t| t.0 == tid).unwrap().1 = state;
    }

    fn idle_task(&mut self, tid: Tid) {
        self.idle_tid = tid;
    }
}
