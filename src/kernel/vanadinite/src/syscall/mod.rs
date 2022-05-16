// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;
pub mod mem;
pub mod misc;
pub mod vmspace;

use librust::{syscalls::Syscall, error::SyscallError};
use crate::{trap::TrapFrame, scheduler::{SCHEDULER, Scheduler}, task::TaskState, mem::paging::VirtualAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Blocked,
    Completed,
}

pub fn handle(frame: &mut TrapFrame, sepc: usize) -> Outcome {
    let task_lock = SCHEDULER.active_on_cpu().unwrap();
    let mut task_lock = task_lock.lock();
    let task = &mut *task_lock;

    let syscall = match Syscall::from_usize(frame.registers.a0) {
        Some(syscall) => syscall,
        None => {
            frame.registers.a0 = usize::from(SyscallError::UnknownSyscall);
            return Outcome::Completed;
        }
    };

    let res = match syscall {
        Syscall::Exit => {
            log::trace!("Task {} ({:?}) exited", task.tid, task.name);
            task.state = TaskState::Dead;
            drop(task_lock);
            SCHEDULER.schedule();
        }
        Syscall::DebugPrint => misc::print(task, VirtualAddress::new(frame.registers.a1), frame.registers.a2),
        Syscall::AllocDmaMemory => mem::alloc_dma_memory(task, frame),
        Syscall::AllocVirtualMemory => mem::alloc_virtual_memory(task, frame),
    };

    match res {
        Ok(()) => frame.a0 = 0,
        Err(e) => frame.a0 = usize::from(e),
    }

    Outcome::Completed
}