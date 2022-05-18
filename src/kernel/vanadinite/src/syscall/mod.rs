// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;
pub mod io;
pub mod mem;
pub mod misc;
pub mod vmspace;

use librust::{syscalls::Syscall, error::SyscallError};
use crate::{trap::{TrapFrame}, scheduler::{SCHEDULER, Scheduler}, task::TaskState, mem::paging::VirtualAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Blocked,
    Completed,
}

pub fn handle(frame: &mut TrapFrame, sepc: usize) -> Outcome {
    let task_lock = SCHEDULER.active_on_cpu().unwrap();
    let mut task_lock = task_lock.lock();
    let task = &mut *task_lock;

    let mut regs = &mut frame.registers;

    let syscall = match Syscall::from_usize(regs.a0) {
        Some(syscall) => syscall,
        None => {
            regs.a0 = usize::from(SyscallError::UnknownSyscall);
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
        Syscall::GetTid => {
            regs.a1 = task.tid.value();
            Ok(())
        },
        Syscall::DebugPrint => misc::print(task, VirtualAddress::new(regs.a1), regs.a2),
        Syscall::AllocDmaMemory => mem::alloc_dma_memory(task, regs),
        Syscall::AllocVirtualMemory => mem::alloc_virtual_memory(task, regs),
        Syscall::ClaimDevice => io::claim_device(task, regs),
        Syscall::CompleteInterrupt => todo!(),
        Syscall::CreateVmspace => vmspace::create_vmspace(task, regs),
        Syscall::AllocVmspaceObject => vmspace::alloc_vmspace_object(task, regs),
        Syscall::SpawnVmspace => vmspace::spawn_vmspace(task, regs),
        Syscall::QueryMemoryCapability => mem::query_mem_cap(task, regs),
        Syscall::QueryMmioCapability => mem::query_mmio_cap(task, regs),
        Syscall::ReadChannel => match channel::read_message(task, regs) {
            Ok(Outcome::Blocked) => {
                let tid = task.tid;
                drop(task_lock);
                SCHEDULER.block(tid);
                return Outcome::Blocked;
            },
            Ok(Outcome::Completed) => Ok(()),
            Err(e) => Err(e),
        },
        Syscall::WriteChannel => channel::send_message(task, regs),
        Syscall::MintCapability => todo!(),
        Syscall::RevokeCapability => todo!(),
    };

    match res {
        Ok(()) => regs.a0 = 0,
        Err(e) => regs.a0 = usize::from(e),
    }

    Outcome::Completed
}