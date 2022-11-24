// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::SchedulerPolicy;
use crate::{sync::Lazy, task::TaskState};
use alloc::collections::VecDeque;
use librust::task::Tid;

pub struct RoundRobinPolicy {
    tids: Lazy<VecDeque<(Tid, TaskState)>>,
}

impl RoundRobinPolicy {
    pub const fn new() -> Self {
        Self { tids: Lazy::new(VecDeque::new) }
    }
}

impl SchedulerPolicy for RoundRobinPolicy {
    fn next(&mut self) -> Option<Tid> {
        match self.tids.is_empty() {
            true => None,
            false => {
                for i in 0..self.tids.len() {
                    self.tids.rotate_left(1);

                    if !matches!(self.tids.front().unwrap().1, TaskState::Ready) {
                        continue;
                    }

                    return Some(self.tids.front().unwrap().0);
                }

                None
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
}
