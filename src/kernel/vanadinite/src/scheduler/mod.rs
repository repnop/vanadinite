// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod round_robin;
pub mod waitqueue;

use crate::csr::satp::Satp;
use crate::mem::paging::SATP_MODE;
use crate::sync::{Lazy, SpinMutex, SpinRwLock};
use crate::task::{Sscratch, TaskState, HART_SSCRATCH};
use crate::N_CPUS;
use crate::{
    csr,
    task::{Context, Task},
    utils::{ticks_per_us, SameHartDeadlockDetection},
};
use alloc::vec::Vec;
use alloc::{collections::BTreeMap, sync::Arc};
use core::cell::Cell;
use core::sync::atomic::AtomicBool;
use core::{
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};
use librust::task::Tid;

use self::round_robin::RoundRobinPolicy;

pub static SCHEDULER: Scheduler = Scheduler::new();
pub static TASKS: TaskList = TaskList::new();

#[thread_local]
pub static CURRENT_TASK: CurrentTask = CurrentTask::empty();

pub struct CurrentTask {
    inner: Cell<*const Task>,
}

impl CurrentTask {
    const fn empty() -> Self {
        Self { inner: Cell::new(core::ptr::null()) }
    }

    #[track_caller]
    fn replace(&self, new: Arc<Task>) -> Arc<Task> {
        assert!(!self.inner.get().is_null(), "`CurrentTask::replace` called while still empty");

        // Safety: `self.inner.get()` is always a valid pointer to an `Arc<Task>`
        let ret = unsafe { Arc::from_raw(self.inner.get()) };
        self.inner.set(Arc::into_raw(new));

        ret
    }

    pub fn tid(&self) -> Tid {
        assert!(!self.inner.get().is_null(), "`CurrentTask::tid` called while still empty");

        // Safety: `self.inner.get()` is always a valid pointer to an `Arc<Task>`
        unsafe { (*self.inner.get()).tid }
    }

    pub fn get(&self) -> Arc<Task> {
        assert!(!self.inner.get().is_null(), "`CurrentTask::get` called while still empty");

        // Safety: `self.inner.get()` is always a valid pointer to an `Arc<Task>`
        let ret = unsafe { Arc::from_raw(self.inner.get()) };

        // Safety: this makes it so that we can do the above `from_raw` while
        // still holding onto the pointer
        unsafe { Arc::increment_strong_count(self.inner.get()) };

        ret
    }

    /// This is expected to only be called once per hart to set the initial task
    /// for the hart. Calling this when there is already a contained task will
    /// panic.
    #[track_caller]
    fn set(&self, task: Arc<Task>) {
        assert!(self.inner.get().is_null());

        self.inner.set(Arc::into_raw(task));
    }
}

pub struct Scheduler {
    inner: Lazy<Vec<SpinMutex<SchedulerInner, SameHartDeadlockDetection>>>,
    // FIXME: actually go back to using this, maybe, at some point?
    #[allow(dead_code)]
    wait_queue: SpinMutex<BTreeMap<Tid, (Arc<Task>, TaskMetadata)>, SameHartDeadlockDetection>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            inner: Lazy::new(|| {
                (0..N_CPUS.load(Ordering::Relaxed)).map(|_| SpinMutex::new(SchedulerInner::new())).collect()
            }),
            wait_queue: SpinMutex::new(BTreeMap::new()),
        }
    }

    pub fn enqueue(&self, task: Task) {
        let (tid, task) = TASKS.insert(task);
        let mut inner = self.queue_for_hart().lock();
        inner.run_queue.insert(tid, (Arc::clone(&task), TaskMetadata::new()));
        inner.policy.task_enqueued(task, TaskMetadata::new());
    }

    pub fn enqueue_with(&self, f: impl FnOnce(Tid) -> Task) {
        let (tid, task) = TASKS.insert_with(f);
        let mut inner = self.queue_for_hart().lock();
        inner.run_queue.insert(tid, (Arc::clone(&task), TaskMetadata::new()));
        inner.policy.task_enqueued(task, TaskMetadata::new());
    }

    // pub fn wake(&self, tid: Tid) {
    //     // FIXME: actually use the blocked queue
    //     let mut inner = self.queue_for_hart().lock();
    //     let (task, mut metadata) = self.wait_queue.lock().remove(&tid).expect("TID not in waitqueue!");
    //     metadata.run_state = TaskState::Ready;

    //     for (i, queue) in self.inner.iter().enumerate() {
    //         let Some(mut queue) = queue.try_lock() else { continue };
    //         if queue.run_queue.len() < inner.run_queue.len() {
    //             log::debug!("Placed now-ready task {} into hart {}'s runqueue", task.name, i);
    //             queue.run_queue.insert(task.tid, (Arc::clone(&task), metadata));
    //             queue.policy.task_enqueued(task, metadata);
    //             return;
    //         }
    //     }

    //     log::debug!("Placed now-ready task {} into our hart's ({}) runqueue", task.name, crate::HART_ID.get());
    //     inner.run_queue.insert(task.tid, (Arc::clone(&task), metadata));
    //     inner.policy.task_enqueued(task, metadata);
    // }

    #[inline(never)]
    pub fn schedule(&self) {
        log::trace!("Scheduling!");
        let mut inner = self.queue_for_hart().lock();
        let SchedulerInner { policy, run_queue, .. } = &mut *inner;
        let current_tid = CURRENT_TASK.tid();

        let (task, metadata) = run_queue.get_mut(&current_tid).expect("TID not in runqueue");
        let (switch_out, out_lock) = unsafe { task.context.raw_locked_parts() };

        log::trace!("[OUT] Task {} [{}] metadata: {:?}", task.name, task.tid, metadata);
        metadata.run_time += csr::time::read() - metadata.last_scheduled_at;

        let tid = policy.next();
        let (to_task, metadata) = run_queue.get_mut(&tid).expect("TID not in runqueue");

        log::trace!("[IN] Task {} [{}] metadata: {:?}", to_task.name, to_task.tid, metadata);

        if tid != current_tid {
            metadata.last_scheduled_at = csr::time::read();

            // Safety: `context_switch` unlocks the mutex
            let (switch_in, in_lock) = unsafe { to_task.context.raw_locked_parts() };
            let satp = Satp {
                root_page_table: to_task.mutable_state.lock().memory_manager.table_phys_address(),
                asid: tid.value() as u16,
                mode: SATP_MODE,
            }
            .as_usize();

            HART_SSCRATCH.set(Sscratch {
                kernel_global_ptr: crate::asm::gp(),
                kernel_thread_local: crate::cpu_local::tp(),
                kernel_stack_top: to_task.kernel_stack,
                scratch_sp: 0,
            });

            drop(CURRENT_TASK.replace(Arc::clone(to_task)));
            drop(inner);

            // Safety: `context_switch` unlocks the mutex
            unsafe { context_switch(switch_out, switch_in, out_lock, in_lock, satp) };
        } else {
            unsafe { (*out_lock).store(false, Ordering::Release) };
            // Keep running the same task if we don't have anything else to do and its not blocked
        }
    }

    /// Begin scheduling on this hart. Requires hart locals to be set up. Automatically spawns an idle task.
    ///
    /// # Safety
    /// This must only be called once at the start of each hart's lifecycle
    pub unsafe fn begin_scheduling(&self) -> ! {
        log::debug!("Scheduling first process!");
        let mut idle_tid = Tid::new(NonZeroUsize::new(usize::MAX).unwrap());
        self.enqueue_with(|tid| {
            idle_tid = tid;
            Task::idle()
        });

        let mut inner = self.queue_for_hart().lock();
        inner.policy.idle_task(idle_tid);

        let next = inner.policy.next();
        let (to_task, _) = inner.run_queue.get_mut(&next).unwrap();

        let to_task = Arc::clone(to_task);
        CURRENT_TASK.set(Arc::clone(&to_task));

        drop(inner);

        // Safety: we promise to be nice
        let (switch_in, in_lock) = unsafe { to_task.context.raw_locked_parts() };
        let satp = Satp {
            root_page_table: to_task.mutable_state.lock().memory_manager.table_phys_address(),
            asid: to_task.tid.value() as u16,
            mode: SATP_MODE,
        }
        .as_usize();

        HART_SSCRATCH.set(Sscratch {
            kernel_global_ptr: crate::asm::gp(),
            kernel_thread_local: crate::cpu_local::tp(),
            kernel_stack_top: to_task.kernel_stack,
            scratch_sp: 0,
        });

        crate::csr::sscratch::write(core::ptr::addr_of!(HART_SSCRATCH) as usize);

        log::debug!("Scheduling first process: {}", to_task.name);

        drop(to_task);
        sbi::timer::set_timer(csr::time::read() + ticks_per_us(10_000, crate::TIMER_FREQ.load(Ordering::Relaxed)))
            .unwrap();
        unsafe { context_load(switch_in, in_lock, satp) };

        unreachable!()
    }

    fn queue_for_hart(&self) -> &SpinMutex<SchedulerInner, SameHartDeadlockDetection> {
        &self.inner[crate::HART_ID.get()]
    }
}

struct SchedulerInner {
    policy: round_robin::RoundRobinPolicy,
    run_queue: BTreeMap<Tid, (Arc<Task>, TaskMetadata)>,
}

impl SchedulerInner {
    fn new() -> Self {
        Self { policy: RoundRobinPolicy::new(), run_queue: BTreeMap::new() }
    }
}

pub struct TaskList {
    map: SpinRwLock<BTreeMap<Tid, Arc<Task>>>,
    next_id: AtomicUsize,
}

impl TaskList {
    pub const fn new() -> Self {
        Self { map: SpinRwLock::new(BTreeMap::new()), next_id: AtomicUsize::new(1) }
    }

    pub fn insert(&self, task: Task) -> (Tid, Arc<Task>) {
        self.insert_with(move |_| task)
    }

    pub fn insert_with(&self, f: impl FnOnce(Tid) -> Task) -> (Tid, Arc<Task>) {
        log::trace!("[TaskList::insert_with] Entered");
        let tid = Tid::new(NonZeroUsize::new(self.next_id.fetch_add(1, Ordering::AcqRel)).unwrap());
        log::trace!("[TaskList::insert_with] Calling f");
        let mut task = f(tid);
        log::trace!("[TaskList::insert_with] task={task:?}");

        task.tid = tid;
        log::trace!("[TaskList::insert_with] Allocating Arc");
        let task: Arc<Task> = Arc::new(task);
        // FIXME: reuse older pids at some point

        log::trace!("[TaskList::insert_with] About to lock");
        log::trace!("Removed task: {:?}", self.map.write().insert(tid, Arc::clone(&task)));
        log::trace!("[TaskList::insert_with] Finished lock");

        (tid, task)
    }

    pub fn remove(&self, tid: Tid) -> Option<Arc<Task>> {
        let res = self.map.write().remove(&tid);

        res
    }

    pub fn get(&self, tid: Tid) -> Option<Arc<Task>> {
        self.map.read().get(&tid).cloned()
    }
}

pub trait SchedulerPolicy {
    fn next(&mut self) -> Tid;
    fn task_enqueued(&mut self, tid: Arc<Task>, metadata: TaskMetadata);
    fn task_dequeued(&mut self, tid: Tid);
    fn task_priority_changed(&mut self, tid: Tid, priority: u16);
    fn task_preempted(&mut self, tid: Tid);

    fn idle_task(&mut self, tid: Tid);
}

#[derive(Debug, Clone, Copy)]
pub struct TaskMetadata {
    pub priority: u16,
    pub run_time: u64,
    pub last_scheduled_at: u64,
    pub run_state: TaskState,
}

impl TaskMetadata {
    pub fn new() -> Self {
        Self { priority: 1, run_time: 0, last_scheduled_at: 0, run_state: TaskState::Ready }
    }
}

#[naked]
unsafe extern "C" fn context_switch(
    /* a0 */ _switch_out: *mut Context,
    /* a1 */ _switch_in: *mut Context,
    /* a2 */ _out_lock: *const AtomicBool,
    /* a3 */ _in_lock: *const AtomicBool,
    /* a4 */ _in_satp: usize,
) {
    #[rustfmt::skip]
    core::arch::asm!("
        sd ra, 0(a0)
        sd sp, 8(a0)

        // This makes things a bit easier to reason about
        addi a0, a0, 16
        sd s0, 0(a0)
        sd s1, 8(a0)
        sd s2, 16(a0)
        sd s3, 24(a0)
        sd s4, 32(a0)
        sd s5, 40(a0)
        sd s6, 48(a0)
        sd s7, 56(a0)
        sd s8, 64(a0)
        sd s9, 72(a0)
        sd s10, 80(a0)
        sd s11, 88(a0)
        
        sd zero, 0(a2)
        fence

        ld ra, 0(a1)
        ld sp, 8(a1)
        addi a1, a1, 16
        ld s0, 0(a1)
        ld s1, 8(a1)
        ld s2, 16(a1)
        ld s3, 24(a1)
        ld s4, 32(a1)
        ld s5, 40(a1)
        ld s6, 48(a1)
        ld s7, 56(a1)
        ld s8, 64(a1)
        ld s9, 72(a1)
        ld s10, 80(a1)
        ld s11, 88(a1)

        csrw satp, a4
        sfence.vma

        sd zero, 0(a3)
        fence

        ret
    ", options(noreturn));
}

#[naked]
unsafe extern "C" fn context_load(
    /* a0 */ _switch_in: *mut Context,
    /* a1 */ _in_lock: *const AtomicBool,
    /* a2 */ _in_satp: usize,
) {
    #[rustfmt::skip]
    core::arch::asm!("
        ld ra, 0(a0)
        ld sp, 8(a0)
        addi a0, a0, 16
        ld s0, 0(a0)
        ld s1, 8(a0)
        ld s2, 16(a0)
        ld s3, 24(a0)
        ld s4, 32(a0)
        ld s5, 40(a0)
        ld s6, 48(a0)
        ld s7, 56(a0)
        ld s8, 64(a0)
        ld s9, 72(a0)
        ld s10, 80(a0)
        ld s11, 88(a0)

        csrw satp, a2
        sfence.vma

        mv t0, zero
        sd t0, 0(a1)
        fence

        ret
    ", options(noreturn));
}

/// # Safety
/// This function assumes that a `Trap` frame is the only thing on the stack at
/// the moment that its called. This should really only ever be called by newly
/// spawned tasks (and potentially the trap handler in the future)
#[naked]
#[no_mangle]
pub unsafe extern "C" fn return_to_usermode() -> ! {
    #[rustfmt::skip]
    core::arch::asm!("
        li t6, 1 << 8
        csrc sstatus, t6
        li t6, 1 << 19
        csrs sstatus, t6
        li t6, 1 << 5
        csrs sstatus, t6

        li t6, 0x222
        csrw sie, t6

        // ld t6, 504(a0)
        // fscsr x0, t6

        ld t6, 0(sp)
        csrw sepc, t6
        
        ld ra, 8(sp)
        // Skip sp for... obvious reasons
        ld gp, 24(sp)
        ld tp, 32(sp)
        ld t0, 40(sp)
        ld t1, 48(sp)
        ld t2, 56(sp)
        ld s0, 64(sp)
        ld s1, 72(sp)
        ld a0, 80(sp)
        ld a1, 88(sp)
        ld a2, 96(sp)
        ld a3, 104(sp)
        ld a4, 112(sp)
        ld a5, 120(sp)
        ld a6, 128(sp)
        ld a7, 136(sp)
        ld s2, 144(sp)
        ld s3, 152(sp)
        ld s4, 160(sp)
        ld s5, 168(sp)
        ld s6, 176(sp)
        ld s7, 184(sp)
        ld s8, 192(sp)
        ld s9, 200(sp)
        ld s10, 208(sp)
        ld s11, 216(sp)
        ld t3, 224(sp)
        ld t4, 232(sp)
        ld t5, 240(sp)
        ld t6, 248(sp)

        // Restore `sp`
        ld sp, 16(sp)

        // FIXME: Restore floating point registers
        // fld f1, 0(a1)
        // fld f2, 8(a1)
        // fld f3, 16(a1)
        // fld f4, 24(a1)
        // fld f5, 32(a1)
        // fld f6, 40(a1)
        // fld f7, 48(a1)
        // fld f8, 56(a1)
        // fld f9, 64(a1)
        // fld f11, 80(a1)
        // fld f12, 88(a1)
        // fld f13, 96(a1)
        // fld f14, 104(a1)
        // fld f15, 112(a1)
        // fld f16, 120(a1)
        // fld f17, 128(a1)
        // fld f18, 136(a1)
        // fld f19, 144(a1)
        // fld f20, 152(a1)
        // fld f21, 160(a1)
        // fld f22, 168(a1)
        // fld f23, 176(a1)
        // fld f24, 184(a1)
        // fld f25, 192(a1)
        // fld f26, 200(a1)
        // fld f27, 208(a1)
        // fld f28, 216(a1)
        // fld f29, 224(a1)
        // fld f30, 232(a1)
        // fld f31, 240(a1)

        sret
    ", options(noreturn));
}
