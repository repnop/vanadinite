// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    cpu_local,
    mem::{
        manager::MemoryManager,
        paging::{
            flags::{EXECUTE, READ, USER, VALID, WRITE},
            VirtualAddress,
        },
    },
    trap::TrapFrame,
    utils::{StaticMut, Units},
};
use elf64::Elf;
use libvanadinite::capabilities::Capability;

pub static PID_COUNTER: PidCounter = PidCounter::new();

cpu_local! {
    pub static THREAD_CONTROL_BLOCK: StaticMut<ThreadControlBlock> = StaticMut::new(ThreadControlBlock::new());
}

pub struct PidCounter(AtomicUsize);

impl PidCounter {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(1))
    }

    pub fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::AcqRel)
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct ThreadControlBlock {
    pub kernel_stack: *mut u8,
    pub kernel_thread_local: *mut u8,
    pub saved_sp: usize,
    pub saved_tp: usize,
    pub kernel_stack_size: usize,
    pub current_process: Option<Process>,
}

impl ThreadControlBlock {
    #[allow(clippy::clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            kernel_stack: core::ptr::null_mut(),
            kernel_thread_local: core::ptr::null_mut(),
            saved_sp: 0,
            saved_tp: 0,
            kernel_stack_size: 0,
            current_process: None,
        }
    }
}

unsafe impl Send for ThreadControlBlock {}
unsafe impl Sync for ThreadControlBlock {}

#[derive(Debug)]
pub struct Process {
    pub pid: usize,
    pub pc: usize,
    pub memory_manager: MemoryManager,
    pub frame: TrapFrame,
    pub state: ProcessState,
    pub capabilities: [Capability; 32],
}

impl Process {
    pub fn load(elf: &Elf) -> Self {
        let mut memory_manager = MemoryManager::new();

        let capabilities = Default::default();

        for header in elf.program_headers().filter(|header| header.r#type == elf64::ProgramSegmentType::Load) {
            memory_manager.alloc_region(
                VirtualAddress::new(header.vaddr as usize),
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
                Some(elf.program_segment_data(&header)),
            );
        }

        memory_manager.alloc_region(VirtualAddress::new(0x7fff0000), 4, USER | READ | WRITE | VALID, None);

        let mut frame = TrapFrame::default();
        frame.registers.sp = 0x7fff0000 + 16.kib();

        Self {
            pid: PID_COUNTER.next(),
            pc: elf.header.entry as usize,
            memory_manager,
            frame,
            state: ProcessState::Running,
            capabilities,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
    Dead,
}

impl ProcessState {
    pub fn is_dead(self) -> bool {
        matches!(self, ProcessState::Dead)
    }
}
