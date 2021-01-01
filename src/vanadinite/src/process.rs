// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::paging::VirtualAddress,
    mem::paging::{Execute, PageTableManager, Read, Sv39PageTable, ToPermissions, User, Write},
    thread_local,
    trap::TrapFrame,
    utils::{StaticMut, Units},
};
use alloc::boxed::Box;
use elf64::Elf;

thread_local! {
    pub static THREAD_CONTROL_BLOCK: StaticMut<ThreadControlBlock> = StaticMut::new(ThreadControlBlock::new());
}

pub static INIT_PROCESS: &[u8] =
    include_bytes!("../../../userspace/template/target/riscv64gc-unknown-none-elf/release/template");

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
    pub pc: extern "C" fn(),
    pub page_table: PageTableManager,
    pub frame: TrapFrame,
    pub state: ProcessState,
}

impl Process {
    pub fn load(elf: &Elf) -> Self {
        let mut page_table = unsafe {
            PageTableManager::new(
                &mut *crate::mem::phys2virt(crate::mem::phys::zalloc_page().as_phys_address()).as_mut_ptr().cast(),
            )
        };

        for header in elf.program_headers().filter(|header| header.r#type == elf64::ProgramSegmentType::Load) {
            page_table.alloc_virtual_range_with_data(
                VirtualAddress::new(header.vaddr as usize),
                header.memory_size as usize,
                match header.flags & 0b111 {
                    0b101 => (User | Read | Execute).to_permissions(),
                    0b011 => (User | Read | Write).to_permissions(),
                    _ => unreachable!(),
                },
                elf.program_segment_data(header),
            );
        }

        page_table.copy_kernel_pages();
        page_table.alloc_virtual_range(VirtualAddress::new(0x7fff0000), 16.kib(), User | Read | Write);

        let mut frame = TrapFrame::default();
        frame.registers.sp = 0x7fff0000 + 16.kib();

        Self {
            pid: 0,
            pc: unsafe { core::mem::transmute(elf.header.entry as usize) },
            page_table,
            frame,
            state: ProcessState::Running,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
    Dead,
}
