// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod round_robin;

use crate::{
    cpu_local, csr,
    task::{Context, Task},
    utils::{ticks_per_us, SameHartDeadlockDetection},
};
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};
use core::{
    cell::Cell,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};
use librust::task::Tid;
use sync::{SpinMutex, SpinRwLock};

pub static SCHEDULER: round_robin::RoundRobinScheduler = round_robin::RoundRobinScheduler::new();
pub static TASKS: TaskList = TaskList::new();

// Used for heuristics in schedulers if they so choose
static N_TASKS: AtomicUsize = AtomicUsize::new(0);

//pub fn init_scheduler(scheduler: Box<dyn Scheduler>) {
//    SCHEDULER.0.write().replace(scheduler).expect("reinitialized scheduler!");
//}

pub struct WakeToken {
    tid: Tid,
    work: Box<dyn FnOnce(&mut Task) + Send>,
}

impl WakeToken {
    pub fn new(tid: Tid, work: impl FnOnce(&mut Task) + Send + 'static) -> Self {
        Self { tid, work: Box::new(work) }
    }
}

impl core::fmt::Debug for WakeToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WakeToken").field("tid", &self.tid).finish_non_exhaustive()
    }
}

pub struct TaskList {
    map: SpinRwLock<BTreeMap<Tid, Arc<SpinMutex<Task, SameHartDeadlockDetection>>>>,
    next_id: AtomicUsize,
}

impl TaskList {
    pub const fn new() -> Self {
        Self { map: SpinRwLock::new(BTreeMap::new()), next_id: AtomicUsize::new(1) }
    }

    pub fn insert(&self, mut task: Task) -> (Tid, Arc<SpinMutex<Task, SameHartDeadlockDetection>>) {
        let tid = Tid::new(NonZeroUsize::new(self.next_id.load(Ordering::Acquire)).unwrap());
        task.tid = tid;
        let task: Arc<SpinMutex<Task, SameHartDeadlockDetection>> = Arc::new(SpinMutex::new(task));
        // FIXME: reuse older pids at some point
        let _ = self.map.write().insert(tid, Arc::clone(&task));
        if self.next_id.fetch_add(1, Ordering::AcqRel) == usize::MAX {
            todo!("something something overflow");
        }

        N_TASKS.fetch_add(1, Ordering::Relaxed);

        (tid, task)
    }

    pub fn remove(&self, tid: Tid) -> Option<Arc<SpinMutex<Task, SameHartDeadlockDetection>>> {
        let res = self.map.write().remove(&tid);

        if res.is_some() {
            N_TASKS.fetch_sub(1, Ordering::Relaxed);
        }

        res
    }

    pub fn get(&self, tid: Tid) -> Option<Arc<SpinMutex<Task, SameHartDeadlockDetection>>> {
        self.map.read().get(&tid).cloned()
    }

    pub fn active_on_cpu(&self) -> Option<Arc<SpinMutex<Task, SameHartDeadlockDetection>>> {
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
    fn enqueue(&self, task: Task) -> Tid;
    fn dequeue(&self, tid: Tid);
    fn block(&self, tid: Tid);
    fn unblock(&self, token: WakeToken);
}

fn sleep() -> ! {
    sbi::timer::set_timer(csr::time::read() + ticks_per_us(10_000, crate::TIMER_FREQ.load(Ordering::Relaxed))).unwrap();
    csr::sie::enable();
    csr::sstatus::enable_interrupts();

    #[rustfmt::skip]
    unsafe {
        core::arch::asm!("
            1: wfi
               j 1b
        ", options(noreturn))
    };
}

#[naked]
#[no_mangle]
unsafe extern "C" fn return_to_usermode(_registers: &Context) -> ! {
    #[rustfmt::skip]
    core::arch::asm!("
        li t0, 1 << 8
        csrc sstatus, t0
        li t0, 1 << 19
        csrs sstatus, t0
        li t0, 1 << 5
        csrs sstatus, t0

        li t0, 0x222
        csrw sie, t0

        ld t0, 504(a0)
        fscsr x0, t0

        ld t0, 512(a0)
        csrw sepc, t0
        
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

        fld f0, 248(a0)
        fld f1, 256(a0)
        fld f2, 264(a0)
        fld f3, 272(a0)
        fld f4, 280(a0)
        fld f5, 288(a0)
        fld f6, 296(a0)
        fld f7, 304(a0)
        fld f8, 312(a0)
        fld f9, 320(a0)
        fld f10, 328(a0)
        fld f11, 336(a0)
        fld f12, 344(a0)
        fld f13, 352(a0)
        fld f14, 360(a0)
        fld f15, 368(a0)
        fld f16, 376(a0)
        fld f17, 384(a0)
        fld f18, 392(a0)
        fld f19, 400(a0)
        fld f20, 408(a0)
        fld f21, 416(a0)
        fld f22, 424(a0)
        fld f23, 432(a0)
        fld f24, 440(a0)
        fld f25, 448(a0)
        fld f26, 456(a0)
        fld f27, 464(a0)
        fld f28, 472(a0)
        fld f29, 480(a0)
        fld f30, 488(a0)
        fld f31, 496(a0)

        ld x10, 72(a0)

        sret
    ", options(noreturn));
}
