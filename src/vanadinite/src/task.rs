// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::{
        manager::{AddressRegionKind, FillOption, MemoryManager},
        paging::{
            flags::{EXECUTE, READ, USER, VALID, WRITE},
            PageSize, VirtualAddress,
        },
    },
    syscall::channel::UserspaceChannel,
    trap::{FloatingPointRegisters, Registers},
    utils::{round_up_to_next, Units},
};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
};
use elf64::{Elf, ProgramSegmentType, Relocation};
use librust::{capabilities::Capability, message::Message, syscalls::channel::ChannelId};

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
    pub channels: BTreeMap<ChannelId, UserspaceChannel>,
    pub capabilities: [Capability; 32],
}

impl Task {
    pub fn load(name: &str, elf: &Elf) -> Self {
        let mut memory_manager = MemoryManager::new();

        let capabilities = Default::default();

        let relocations = elf
            .relocations()
            .map(|reloc| match reloc {
                Relocation::Rel(rel) => (VirtualAddress::new(rel.offset as usize), reloc),
                Relocation::Rela(rela) => (VirtualAddress::new(rela.offset as usize), reloc),
            })
            .collect::<BTreeMap<VirtualAddress, Relocation>>();

        // Try to estimate the size of the buffer we'll need
        let (total_size, max_file_size) = elf.load_segments().fold((0, 0), |(sum, max), header| {
            (
                round_up_to_next(sum, header.align as usize)
                    + round_up_to_next(header.memory_size as usize, header.align as usize),
                max.max(header.file_size as usize),
            )
        });

        // See if we have a RELRO section to fix up
        let relro = elf
            .program_headers()
            .find(|header| header.r#type == ProgramSegmentType::GnuRelro)
            .map(|header| header.vaddr as usize);

        assert_eq!(total_size % 4.kib(), 0, "load segments not totally whole pages");

        // FIXME: first segment load might be `2.mib()`, so prob need a `match` here
        let task_load_base = memory_manager.find_free_region(PageSize::Kilopage, total_size / 4.kib());
        let mut segment_offset = task_load_base;
        let mut segment_data = alloc::vec![0; max_file_size];
        let mut pc = VirtualAddress::new(0);
        let elf_entry = VirtualAddress::new(elf.header.entry as usize);

        for header in elf.load_segments() {
            let align = header.align as usize;
            let mem_size = header.memory_size as usize;
            let vaddr = header.vaddr as usize;
            let file_size = header.file_size as usize;
            let is_relro = Some(vaddr) == relro;

            assert!(align.is_power_of_two(), "ELF segment alignment isn't a power of two!");
            assert!(mem_size >= file_size, "ELF segment has less data in memory than in the file?");

            // Need to align-up the segment offset we were given here
            let segment_load_base = VirtualAddress::new(round_up_to_next(segment_offset.as_usize(), align));
            // Grab the bottom bits that we need to start writing data at
            let segment_load_offset = vaddr & (align - 1);
            // The total segment length we need is the size in the file + the
            // above offset since we start at the aligned address
            let segment_len = file_size + segment_load_offset;
            // The total size in memory rounded up to the next alignment for the
            // segment
            let region_size = round_up_to_next(mem_size + segment_load_offset, align);

            if segment_data.len() < segment_len {
                segment_data.resize(segment_len, 0);
            }

            // Copy the segment data starting at the offset
            segment_data[segment_load_offset..][..file_size].copy_from_slice(elf.program_segment_data(&header));

            // We use these values to key off of some information (e.g.
            // relocation calculations and calculating the PC)
            let raw_segment_start = VirtualAddress::new(header.vaddr as usize);
            let raw_segment_end = raw_segment_start.add(header.memory_size as usize);
            let raw_segment_range = raw_segment_start..raw_segment_end;

            // The real PC needs calculated from the offset, so we check to see
            // if this is the segment that contains the entry point
            if raw_segment_range.contains(&elf_entry) {
                let offset = elf_entry.as_usize() - raw_segment_start.as_usize() + segment_load_offset;
                pc = segment_load_base.add(offset);
            }

            // Find any relocations and fix them up before we write the memory
            // so we don't need to deal with the `UniquePhysicalRegion` which
            // doesn't play nice with arbitrary indexing since the physical
            // pages aren't guaranteed to be contiguous here so we can reuse
            // memory
            for (_, relocation) in relocations.range(raw_segment_start..raw_segment_end) {
                match relocation {
                    Relocation::Rel(_) => todo!("rel relocations"),
                    Relocation::Rela(rela) => {
                        let offset_into = rela.offset as usize - raw_segment_start.as_usize() + segment_load_offset;

                        match rela.r#type {
                            // RELATIVE
                            3 => {
                                // FIXME: Should prob check for negative addends?
                                let fixup = task_load_base.as_usize() + rela.addend as usize;
                                segment_data[offset_into..][..8].copy_from_slice(&fixup.to_le_bytes());
                            }
                            n => todo!("relocation type: {}", n),
                        }
                    }
                }
            }

            // RELRO will override any other permission flags here, so check to
            // see if the region we just processed is the RELRO segment
            let (kind, flags) = match (is_relro, header.flags) {
                (true, _) => (AddressRegionKind::ReadOnly, USER | READ | VALID),
                (false, 0b101) => (AddressRegionKind::Text, USER | READ | EXECUTE | VALID),
                (false, 0b110) => (AddressRegionKind::Data, USER | READ | WRITE | VALID),
                (false, 0b100) => (AddressRegionKind::ReadOnly, USER | READ | VALID),
                (false, flags) => unreachable!("flags: {:#b}", flags),
            };

            memory_manager.alloc_region(
                Some(segment_load_base),
                PageSize::Kilopage,
                region_size / 4.kib(),
                flags,
                FillOption::Data(&segment_data[..segment_len]),
                kind,
            );

            segment_offset = segment_load_base.add(region_size);
        }

        let tls = elf.program_headers().find(|header| header.r#type == elf64::ProgramSegmentType::Tls).map(|header| {
            // This is mostly the same as the above, just force 4 KiB alignment
            // because its not like we can have 8-byte aligned pages.
            //
            // TODO: `.tbss`?
            let n_pages_needed = round_up_to_next(header.memory_size as usize, 4.kib()) / 4.kib();
            let tls_base = memory_manager.find_free_region(PageSize::Kilopage, n_pages_needed);

            // This might actually not be necessary, since in the end the
            // thread-local loads are done with `tp + offset` but in case this
            // is important for any possible TLS relocations later, keeping it
            // the same as above
            let segment_load_offset = (header.vaddr & (4.kib() - 1)) as usize;
            let segment_len = header.file_size as usize + segment_load_offset;

            if segment_data.len() < segment_len {
                segment_data.resize(segment_len, 0);
            }

            let segment_file_size = header.file_size as usize;
            segment_data[segment_load_offset..][..segment_file_size].copy_from_slice(elf.program_segment_data(&header));
            segment_data[segment_load_offset..][segment_file_size..].fill(0);

            memory_manager.alloc_region(
                Some(tls_base),
                PageSize::Kilopage,
                n_pages_needed,
                USER | READ | WRITE | VALID,
                FillOption::Data(&segment_data[..segment_len]),
                AddressRegionKind::Tls,
            );

            tls_base.add(segment_load_offset).as_usize()
        });

        // We guard the stack on both ends, though a stack underflow is
        // unlikely, but better to be safe than sorry!
        let sp = memory_manager
            .alloc_guarded_region(
                PageSize::Kilopage,
                4,
                USER | READ | WRITE | VALID,
                FillOption::Unitialized,
                AddressRegionKind::Stack,
            )
            .add(16.kib());

        log::info!("\n{:#?}", memory_manager.address_map_debug());

        let context = Context {
            pc: pc.as_usize(),
            gp_regs: Registers { sp: sp.as_usize(), tp: tls.unwrap_or(0), ..Default::default() },
            fp_regs: FloatingPointRegisters::default(),
        };

        Self {
            name: Box::from(name),
            context,
            memory_manager,
            state: TaskState::Running,
            channels: BTreeMap::new(),
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
