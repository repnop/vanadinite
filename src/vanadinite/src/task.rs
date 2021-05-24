// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::{
        manager::{FillOption, MemoryManager},
        paging::{
            flags::{EXECUTE, READ, USER, VALID, WRITE},
            PageSize, VirtualAddress,
        },
    },
    trap::{FloatingPointRegisters, Registers},
    utils::{round_up_to_next, Units},
};
use alloc::{boxed::Box, collections::VecDeque};
use elf64::Elf;
use librust::{capabilities::Capability, message::Message};

#[derive(Debug)]
#[repr(C)]
pub struct ThreadControlBlock {
    pub kernel_stack: *mut u8,
    pub kernel_thread_local: *mut u8,
    pub saved_sp: usize,
    pub saved_tp: usize,
    pub kernel_stack_size: usize,
}

impl ThreadControlBlock {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            kernel_stack: core::ptr::null_mut(),
            kernel_thread_local: core::ptr::null_mut(),
            saved_sp: 0,
            saved_tp: 0,
            kernel_stack_size: 0,
        }
    }

    /// # Safety
    /// This assumes that the pointer to the [`ThreadControlBlock`] has been set
    /// in the `sstatus` register
    pub unsafe fn the() -> *mut Self {
        let ret;
        asm!("csrr {}, sstatus", out(reg) ret);
        ret
    }
}

unsafe impl Send for ThreadControlBlock {}
unsafe impl Sync for ThreadControlBlock {}

#[derive(Debug, Clone)]
pub struct Context {
    pub pc: usize,
    pub gp_regs: Registers,
    pub fp_regs: FloatingPointRegisters,
}

pub struct Task {
    pub name: Box<str>,
    pub context: Context,
    pub memory_manager: MemoryManager,
    pub state: TaskState,
    pub message_queue: VecDeque<Message>,
    pub capabilities: [Capability; 32],
}

impl Task {
    pub fn load(name: &str, elf: &Elf) -> Self {
        let mut memory_manager = MemoryManager::new();

        let capabilities = Default::default();

        for header in elf.program_headers().filter(|header| header.r#type == elf64::ProgramSegmentType::Load) {
            log::debug!("header: {:?}", header);
            memory_manager.alloc_region(
                Some(VirtualAddress::new(header.vaddr as usize)),
                PageSize::Kilopage,
                match (header.memory_size as usize / 4.kib(), header.memory_size as usize % 4.kib()) {
                    (n, 0) => n,
                    (n, _) => n + 1,
                },
                match header.flags {
                    0b101 => USER | READ | EXECUTE | VALID,
                    0b110 => USER | READ | WRITE | VALID,
                    0b100 => USER | READ | VALID,
                    flags => unreachable!("flags: {:#b}", flags),
                },
                FillOption::Data(elf.program_segment_data(&header)),
            );
        }

        let tls = elf.program_headers().find(|header| header.r#type == elf64::ProgramSegmentType::Tls).map(|header| {
            log::debug!("header: {:?}", header);
            memory_manager
                .alloc_region(
                    None,
                    PageSize::Kilopage,
                    round_up_to_next(header.memory_size as usize, 4096) / 4096,
                    USER | READ | WRITE | VALID,
                    FillOption::Data(elf.program_segment_data(&header)),
                )
                .as_usize()
        });

        memory_manager.alloc_region(
            Some(VirtualAddress::new(0x7fff0000)),
            PageSize::Kilopage,
            4,
            USER | READ | WRITE | VALID,
            FillOption::Unitialized,
        );

        memory_manager.guard(VirtualAddress::new(0x7fff0000 - 4.kib()));
        memory_manager.guard(VirtualAddress::new(0x7fff0000 + 16.kib()));

        let context = Context {
            pc: elf.header.entry as usize,
            gp_regs: Registers { sp: 0x7fff0000 + 16.kib(), tp: tls.unwrap_or(0), ..Default::default() },
            fp_regs: FloatingPointRegisters::default(),
        };

        Self {
            name: Box::from(name),
            context,
            memory_manager,
            state: TaskState::Running,
            message_queue: VecDeque::new(),
            capabilities,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskState {
    Blocked,
    Dead,
    Running,
}

impl TaskState {
    pub fn is_dead(self) -> bool {
        matches!(self, TaskState::Dead)
    }
}
