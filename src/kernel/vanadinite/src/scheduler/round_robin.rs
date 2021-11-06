// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::Ordering;

use super::{Scheduler, Task, Tid, WakeToken, CURRENT_TASK, TASKS};
use crate::{
    csr::{self, satp::Satp},
    mem::{self, paging::SATP_MODE},
    task::TaskState,
    utils::{ticks_per_us, SameHartDeadlockDetection},
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use sync::{Lazy, SpinMutex};

struct QueuedTask {
    tid: Tid,
    task: Arc<SpinMutex<Task, SameHartDeadlockDetection>>,
    token: Option<WakeToken>,
}

pub struct RoundRobinScheduler {
    blocked: Lazy<SpinMutex<VecDeque<QueuedTask>, SameHartDeadlockDetection>>,
    queues: Lazy<Vec<SpinMutex<VecDeque<QueuedTask>, SameHartDeadlockDetection>>>,
}

impl RoundRobinScheduler {
    pub const fn new() -> Self {
        Self {
            blocked: Lazy::new(|| SpinMutex::new(VecDeque::new())),
            queues: Lazy::new(|| {
                let n_cpus = crate::N_CPUS.load(core::sync::atomic::Ordering::Acquire);
                let mut v = Vec::with_capacity(n_cpus);

                for _ in 0..n_cpus {
                    v.push(SpinMutex::new(VecDeque::with_capacity(16)));
                }

                v
            }),
        }
    }

    fn current_queue(&self) -> &SpinMutex<VecDeque<QueuedTask>, SameHartDeadlockDetection> {
        let current_hart = crate::HART_ID.get();
        &self.queues[current_hart]
    }
}

impl Scheduler for RoundRobinScheduler {
    fn schedule(&self) -> ! {
        log::debug!("Starting scheduling");
        let mut queue = self.current_queue().lock();
        let queue_len = queue.len();

        if queue.len() > 1 {
            queue.rotate_left(1);
        }

        let to_run = loop {
            let queued_task = match queue.front_mut() {
                Some(queued_task) => queued_task,
                None => break None,
            };

            let state = queued_task.task.lock().state;

            match state {
                TaskState::Blocked if queue_len > 1 => queue.rotate_left(1),
                TaskState::Blocked => break None,
                TaskState::Dead => drop(queue.pop_front()),
                TaskState::Running => {
                    break Some(queued_task);
                }
            }
        };

        match to_run {
            Some(queued_task) => {
                let mut task = queued_task.task.lock();

                if let Some(token) = queued_task.token.take() {
                    (token.work)(&mut task);
                }

                let root_page_table = task.memory_manager.table_phys_address();
                let context = task.context.clone();
                let tid = queued_task.tid;

                CURRENT_TASK.set(Some(queued_task.tid));

                log::debug!("Scheduling {:?}, pc: {:#p}", task.name, task.context.pc as *mut u8);

                // !! RELEASE LOCKS BEFORE CONTEXT SWITCHING !!
                drop(task);
                drop(queue);

                sbi::timer::set_timer(
                    csr::time::read() + ticks_per_us(10_000, crate::TIMER_FREQ.load(Ordering::Relaxed)),
                )
                .unwrap();

                csr::satp::write(Satp { mode: SATP_MODE, asid: tid.value() as u16, root_page_table });
                mem::sfence(None, None);

                unsafe { super::return_to_usermode(&context) }
            }
            None => {
                // !! RELEASE LOCK BEFORE CONTEXT SWITCHING !!
                drop(queue);

                log::debug!("No work to do, sleeping :(");

                mem::sfence(None, None);
                CURRENT_TASK.set(None);

                super::sleep()
            }
        }
    }

    fn enqueue(&self, task: Task) -> Tid {
        let (tid, task) = TASKS.insert(task);

        log::debug!("Trying to enqueue task");
        let selected = self.queues.iter().min_by_key(|queue| queue.lock().len()).unwrap_or(&self.queues[0]);
        selected.lock().push_back(QueuedTask { tid, task, token: None });
        log::debug!("Enqueued task");

        tid
    }

    fn dequeue(&self, tid: Tid) {
        let mut queue = self.current_queue().lock();
        if let Some(index) = queue.iter().position(|t| t.tid == tid) {
            queue.remove(index);
        }
    }

    fn block(&self, tid: Tid) {
        let mut queue = self.current_queue().lock();
        let index = queue.iter().position(|t| t.tid == tid).expect("blocking task not on current hart");
        let task = queue.remove(index).unwrap();
        self.blocked.lock().push_back(task);
    }

    fn unblock(&self, token: WakeToken) {
        let mut blocked = self.blocked.lock();
        let index = blocked.iter().position(|t| t.tid == token.tid).expect("trying to wake a non-blocked task");
        let mut task = blocked.remove(index).unwrap();
        drop(blocked);

        task.token = Some(token);

        let selected = self.queues.iter().min_by_key(|queue| queue.lock().len()).unwrap_or(&self.queues[0]);
        selected.lock().push_back(task);
    }
}
