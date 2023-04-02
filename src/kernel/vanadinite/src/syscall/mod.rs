// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod capabilities;
pub mod channel;
pub mod io;
pub mod mem;
pub mod misc;
pub mod vmspace;

use crate::{
    mem::paging::VirtualAddress,
    scheduler::{CURRENT_TASK, SCHEDULER},
    task::TaskState,
    trap::TrapFrame,
};
use librust::{error::SyscallError, syscalls::Syscall};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Blocked,
    Completed,
}

pub fn handle(frame: &mut TrapFrame) {
    let task = CURRENT_TASK.get();
    let task = &*task;

    let mut regs = &mut frame.registers;

    let syscall = match Syscall::from_usize(regs.a0) {
        Some(syscall) => syscall,
        None => {
            regs.a0 = usize::from(SyscallError::UnknownSyscall);
            return;
        }
    };

    let res = match syscall {
        Syscall::Exit => {
            task.mutable_state.lock().state = TaskState::Dead;
            log::trace!("Task {} ({:?}) exited", task.tid, task.name);
            SCHEDULER.schedule();
            unreachable!("Dead task [{}] {} rescheduled?", task.tid, task.name)
        }
        Syscall::GetTid => {
            regs.a1 = task.tid.value();
            Ok(())
        }
        Syscall::DebugPrint => misc::print(task, VirtualAddress::new(regs.a1), regs.a2),
        Syscall::AllocDmaMemory => mem::alloc_dma_memory(task, regs),
        Syscall::AllocVirtualMemory => mem::alloc_virtual_memory(task, regs),
        Syscall::ClaimDevice => io::claim_device(task, regs),
        Syscall::CompleteInterrupt => io::complete_interrupt(task, regs),
        Syscall::CreateVmspace => vmspace::create_vmspace(task, regs),
        Syscall::AllocVmspaceObject => vmspace::alloc_vmspace_object(task, regs),
        Syscall::SpawnVmspace => vmspace::spawn_vmspace(task, regs),
        Syscall::QueryMemoryCapability => mem::query_mem_cap(task, regs),
        Syscall::QueryMmioCapability => mem::query_mmio_cap(task, regs),
        Syscall::ReadChannel => channel::read_message(task, regs),
        Syscall::WriteChannel => channel::send_message(task, regs),
        Syscall::MintCapability => todo!(),
        Syscall::RevokeCapability => todo!(),
        Syscall::EnableNotifications => Ok(task.mutable_state.lock().subscribes_to_events = true),
        Syscall::DeleteCapability => capabilities::delete(task, regs),
    };

    match res {
        Ok(()) => regs.a0 = 0,
        Err(e) => regs.a0 = usize::from(e),
    }
}
