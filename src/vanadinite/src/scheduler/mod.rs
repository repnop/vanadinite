// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod round_robin;

use crate::{
    cpu_local,
    sync::{SpinMutex, SpinRwLock},
    task::Task,
    utils::ticks_per_us,
};
use alloc::{collections::BTreeMap, sync::Arc};
use core::{
    cell::Cell,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};
use librust::task::Tid;

pub static SCHEDULER: round_robin::RoundRobinScheduler = round_robin::RoundRobinScheduler::new();
pub static TASKS: TaskList = TaskList::new();

// Used for heuristics in schedulers if they so choose
static N_TASKS: AtomicUsize = AtomicUsize::new(0);

//pub fn init_scheduler(scheduler: Box<dyn Scheduler>) {
//    SCHEDULER.0.write().replace(scheduler).expect("reinitialized scheduler!");
//}

pub struct TaskList {
    map: SpinRwLock<BTreeMap<Tid, Arc<SpinMutex<Task>>>>,
    next_id: AtomicUsize,
}

impl TaskList {
    pub const fn new() -> Self {
        Self { map: SpinRwLock::new(BTreeMap::new()), next_id: AtomicUsize::new(1) }
    }

    pub fn insert(&self, task: Task) -> (Tid, Arc<SpinMutex<Task>>) {
        let tid = Tid::new(NonZeroUsize::new(self.next_id.load(Ordering::Acquire)).unwrap());
        let task = Arc::new(SpinMutex::new(task));
        // FIXME: reuse older pids at some point
        let _ = self.map.write().insert(tid, Arc::clone(&task));
        if self.next_id.fetch_add(1, Ordering::AcqRel) == usize::MAX {
            todo!("something something overflow");
        }

        N_TASKS.fetch_add(1, Ordering::Relaxed);

        (tid, task)
    }

    pub fn remove(&self, tid: Tid) -> Option<Arc<SpinMutex<Task>>> {
        let res = self.map.write().remove(&tid);

        if res.is_some() {
            N_TASKS.fetch_sub(1, Ordering::Relaxed);
        }

        res
    }

    pub fn get(&self, tid: Tid) -> Option<Arc<SpinMutex<Task>>> {
        self.map.read().get(&tid).cloned()
    }

    pub fn active_on_cpu(&self) -> Option<Arc<SpinMutex<Task>>> {
        match CURRENT_TASK.get() {
            Some(tid) => self.get(tid),
            None => None,
        }
    }
}

cpu_local! {
    pub static CURRENT_TASK: Cell<Option<Tid>> = Cell::new(None);
}

pub trait Scheduler: Send {
    fn schedule(&self) -> !;
    fn enqueue(&self, task: Task);
    fn dequeue(&self, tid: Tid);
}

fn sleep() -> ! {
    sbi::timer::set_timer(ticks_per_us(10_000, crate::TIMER_FREQ.load(Ordering::Relaxed)) as u64).unwrap();
    crate::csr::sie::enable();
    crate::csr::sstatus::enable_interrupts();

    #[rustfmt::skip]
    unsafe {
        asm!("
            1: wfi
               j 1b
        ", options(noreturn))
    };
}

#[naked]
#[no_mangle]
unsafe extern "C" fn return_to_usermode(_registers: &crate::trap::Registers, _sepc: usize) -> ! {
    #[rustfmt::skip]
    asm!("
        csrw sepc, a1

        li t0, 1 << 8
        csrc sstatus, t0
        li t0, 1 << 19
        csrs sstatus, t0
        li t0, 1 << 5
        csrs sstatus, t0
        
        ld x1, 0(a0)
        ld x2, 8(a0)
        ld x3, 16(a0)
        ld x4, 24(a0)
        ld x5, 32(a0)
        ld x6, 40(a0)
        ld x7, 48(a0)
        ld x8, 56(a0)
        ld x9, 64(a0)
        ld x11, 80(a0)
        ld x12, 88(a0)
        ld x13, 96(a0)
        ld x14, 104(a0)
        ld x15, 112(a0)
        ld x16, 120(a0)
        ld x17, 128(a0)
        ld x18, 136(a0)
        ld x19, 144(a0)
        ld x20, 152(a0)
        ld x21, 160(a0)
        ld x22, 168(a0)
        ld x23, 176(a0)
        ld x24, 184(a0)
        ld x25, 192(a0)
        ld x26, 200(a0)
        ld x27, 208(a0)
        ld x28, 216(a0)
        ld x29, 224(a0)
        ld x30, 232(a0)
        ld x31, 240(a0)

        ld x10, 72(a0)

        sret
    ", options(noreturn));
}
