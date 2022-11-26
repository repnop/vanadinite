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
use crate::sync::{SpinMutex, SpinRwLock};
use crate::task::{Sscratch, TaskState, HART_SSCRATCH};
use crate::N_CPUS;
use crate::{
    csr,
    task::{Context, Task},
    utils::{ticks_per_us, SameHartDeadlockDetection},
};
use alloc::{collections::BTreeMap, sync::Arc};
use core::cell::RefCell;
use core::{
    num::NonZeroUsize,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use librust::task::Tid;

use self::round_robin::RoundRobinPolicy;

pub static SCHEDULER: Scheduler = Scheduler::new();
pub static TASKS: TaskList = TaskList::new();

#[thread_local]
pub static CURRENT_TASK: RefCell<CurrentTask> = RefCell::new(CurrentTask::empty());

#[thread_local]
static IDLE_TASK: RefCell<CurrentTask> = RefCell::new(CurrentTask::empty());

pub struct CurrentTask {
    inner: *const Task,
}

impl CurrentTask {
    const fn empty() -> Self {
        Self { inner: core::ptr::null() }
    }

    #[track_caller]
    fn replace(&mut self, new: Arc<Task>) -> Arc<Task> {
        assert!(!self.inner.is_null(), "`CurrentTask::replace` called while still empty");

        let ret = unsafe { Arc::from_raw(self.inner) };
        self.inner = Arc::into_raw(new);

        ret
    }

    pub fn get(&self) -> Arc<Task> {
        assert!(!self.inner.is_null(), "`CurrentTask::get` called while still empty");

        let ret = unsafe { Arc::from_raw(self.inner) };

        // Safety: this makes it so that we can do the above `from_raw` while
        // still holding onto the pointer
        unsafe { Arc::increment_strong_count(self.inner) };

        ret
    }

    /// This is expected to only be called once per hart to set the initial task
    /// for the hart. Calling this when there is already a contained task will
    /// panic.
    #[track_caller]
    fn set(&mut self, task: Arc<Task>) {
        assert!(self.inner.is_null());

        self.inner = Arc::into_raw(task);
    }
}

impl core::ops::Deref for CurrentTask {
    type Target = Task;

    #[track_caller]
    fn deref(&self) -> &Self::Target {
        assert!(!self.inner.is_null(), "`CurrentTask::set` never called!");

        // Safety: `self.inner` always points to a valid `T` as `Arc::into_raw`
        // essentially "leaks" a strong count until we `Arc::from_raw`
        unsafe { &*self.inner }
    }
}

pub struct Scheduler {
    inner: SpinMutex<SchedulerInner, SameHartDeadlockDetection>,
    total_task_count: AtomicU64,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self { inner: SpinMutex::new(SchedulerInner::new()), total_task_count: AtomicU64::new(0) }
    }

    pub fn enqueue(&self, mut task: Task) {
        task.mutable_state.get_mut().state = TaskState::Ready;
        let (tid, task) = TASKS.insert(task);
        let mut inner = self.inner.lock();
        inner.run_queue.insert(tid, (task.clone(), TaskMetadata::new()));
        inner.policy.task_enqueued(tid, TaskMetadata::new());

        if CURRENT_TASK.borrow().inner.is_null() {
            CURRENT_TASK.borrow_mut().set(task);
        }
    }

    pub fn enqueue_with(&self, f: impl FnOnce(Tid) -> Task) {
        let (tid, task) = TASKS.insert_with(|tid| {
            let mut task = f(tid);
            task.mutable_state.get_mut().state = TaskState::Ready;
            task
        });

        let mut inner = self.inner.lock();

        inner.run_queue.insert(tid, (task.clone(), TaskMetadata::new()));
        inner.policy.task_enqueued(tid, TaskMetadata::new());

        if CURRENT_TASK.borrow().inner.is_null() {
            CURRENT_TASK.borrow_mut().set(task);
        }
    }

    pub fn wake(&self, tid: Tid) {
        // FIXME: actually use the blocked queue
        let mut inner = self.inner.lock();
        inner.run_queue.get_mut(&tid).expect("TID not in runqueue!").1.run_state = TaskState::Ready;
        inner.policy.update_state(tid, TaskState::Ready);
    }

    #[inline(never)]
    pub fn schedule(&self, next_state: TaskState) {
        log::debug!("Scheduling!");
        let mut inner = self.inner.lock();
        let SchedulerInner { policy, run_queue, .. } = &mut *inner;
        let current_tid = CURRENT_TASK.borrow().tid;

        let (task, metadata) = run_queue.get_mut(&current_tid).expect("TID not in runqueue");
        log::debug!("[OUT] Task {} [{}] metadata: {:?}", task.name, task.tid, metadata);
        assert_eq!(metadata.run_state, TaskState::Running);
        metadata.run_state = next_state;
        policy.update_state(current_tid, next_state);

        let (switch_out, out_lock) = unsafe { task.context.lock_into_parts() };
        let tid = policy.next().expect("TODO: better idle task situation");
        let (to_task, metadata) = run_queue.get_mut(&tid).expect("TID not in runqueue");

        log::debug!("[IN] Task {} [{}] metadata: {:?}", to_task.name, to_task.tid, metadata);

        log::debug!("Scheduling {} next", to_task.name);

        if tid != current_tid {
            assert_eq!(metadata.run_state, TaskState::Ready);
            metadata.run_state = TaskState::Running;
            policy.update_state(tid, TaskState::Running);

            // Safety: we promise to be nice
            let (switch_in, in_lock) = unsafe { to_task.context.lock_into_parts() };
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

            // This probably isn't necessary? but just for now make sure its
            // set properly on every switch
            crate::csr::sscratch::write(core::ptr::addr_of!(HART_SSCRATCH) as usize);

            let _ = CURRENT_TASK.borrow_mut().replace(to_task.clone());

            drop(inner);

            unsafe { context_switch(switch_out, switch_in, out_lock, in_lock, satp) };
        } else {
            unsafe { (*out_lock).store(0, Ordering::Release) };
            assert_eq!(next_state, TaskState::Ready);
            metadata.run_state = TaskState::Running;
            inner.policy.update_state(current_tid, TaskState::Running);
            // Keep running the same task if we don't have anything else to do and its not blocked
        }
    }

    /// # Safety
    /// This must only be called once at the start of each hart's lifecycle
    pub unsafe fn begin_scheduling(&self) -> ! {
        let to_task = CURRENT_TASK.borrow();

        let mut inner = self.inner.lock();
        let (_, metadata) = inner.run_queue.get_mut(&to_task.tid).unwrap();
        assert_eq!(metadata.run_state, TaskState::Ready);
        metadata.run_state = TaskState::Running;
        inner.policy.update_state(to_task.tid, TaskState::Running);

        drop(inner);

        // Safety: we promise to be nice
        let (switch_in, in_lock) = unsafe { to_task.context.lock_into_parts() };
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

        // This probably isn't necessary? but just for now make sure its
        // set properly on every switch
        crate::csr::sscratch::write(core::ptr::addr_of!(HART_SSCRATCH) as usize);

        log::debug!("Scheduling first process: {}", to_task.name);

        drop(to_task);
        sbi::timer::set_timer(csr::time::read() + ticks_per_us(10_000, crate::TIMER_FREQ.load(Ordering::Relaxed)))
            .unwrap();
        unsafe { context_load(switch_in, in_lock, satp) };

        unreachable!()
    }
}

struct SchedulerInner {
    policy: round_robin::RoundRobinPolicy,
    run_queue: BTreeMap<Tid, (Arc<Task>, TaskMetadata)>,
    wait_queue: BTreeMap<Tid, (Arc<Task>, TaskMetadata)>,
}

impl SchedulerInner {
    const fn new() -> Self {
        Self { policy: RoundRobinPolicy::new(), run_queue: BTreeMap::new(), wait_queue: BTreeMap::new() }
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

    pub fn insert(&self, mut task: Task) -> (Tid, Arc<Task>) {
        let tid = Tid::new(NonZeroUsize::new(self.next_id.load(Ordering::Acquire)).unwrap());
        task.tid = tid;
        let task: Arc<Task> = Arc::new(task);
        // FIXME: reuse older pids at some point
        let _ = self.map.write().insert(tid, Arc::clone(&task));
        if self.next_id.fetch_add(1, Ordering::AcqRel) == usize::MAX {
            todo!("something something overflow");
        }

        (tid, task)
    }

    pub fn insert_with(&self, f: impl FnOnce(Tid) -> Task) -> (Tid, Arc<Task>) {
        let tid = Tid::new(NonZeroUsize::new(self.next_id.load(Ordering::Acquire)).unwrap());
        self.insert(f(tid))
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
    fn next(&mut self) -> Option<Tid>;
    fn task_enqueued(&mut self, tid: Tid, metadata: TaskMetadata);
    fn task_dequeued(&mut self, tid: Tid);
    fn task_priority_changed(&mut self, tid: Tid, priority: u16);
    fn task_preempted(&mut self, tid: Tid);
    fn update_state(&mut self, tid: Tid, state: TaskState);
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
    /* a2 */ _out_lock: *const AtomicU64,
    /* a3 */ _in_lock: *const AtomicU64,
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
    /* a1 */ _in_lock: *const AtomicU64,
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
