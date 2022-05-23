// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;

use crate::{
    capabilities::{Capability, CapabilityResource, CapabilitySpace},
    mem::{
        manager::{AddressRegionKind, FillOption, MemoryManager, RegionDescription},
        paging::{
            flags::{EXECUTE, READ, USER, VALID, WRITE},
            PageSize, VirtualAddress,
        },
    },
    platform::FDT,
    syscall::{channel::UserspaceChannel, vmspace::VmspaceObject},
    trap::{FloatingPointRegisters, GeneralRegisters},
    utils::{round_up_to_next, Units},
};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet, VecDeque},
    vec::Vec,
};
use elf64::{Elf, ProgramSegmentType, Relocation};
use fdt::Fdt;
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    syscalls::{channel::KERNEL_CHANNEL, vmspace::VmspaceObjectId},
    task::Tid,
};

#[derive(Debug)]
#[repr(C)]
pub struct ThreadControlBlock {
    pub kernel_stack: *mut u8,
    pub kernel_thread_local: *mut u8,
    pub kernel_global_ptr: *mut u8,
    pub saved_sp: usize,
    pub saved_tp: usize,
    pub saved_gp: usize,
    pub kernel_stack_size: usize,
}

impl ThreadControlBlock {
    pub fn new() -> Self {
        Self {
            kernel_stack: core::ptr::null_mut(),
            kernel_thread_local: core::ptr::null_mut(),
            kernel_global_ptr: core::ptr::null_mut(),
            saved_sp: 0,
            saved_tp: 0,
            saved_gp: 0,
            kernel_stack_size: 0,
        }
    }

    /// # Safety
    /// This assumes that the pointer to the [`ThreadControlBlock`] has been set
    /// in the `sstatus` register
    pub unsafe fn the() -> *mut Self {
        let ret;
        core::arch::asm!("csrr {}, sstatus", out(reg) ret);
        ret
    }
}

unsafe impl Send for ThreadControlBlock {}
unsafe impl Sync for ThreadControlBlock {}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Context {
    pub gp_regs: GeneralRegisters,
    pub fp_regs: FloatingPointRegisters,
    pub pc: usize,
}

pub struct Task {
    pub tid: Tid,
    pub name: Box<str>,
    pub context: Context,
    pub memory_manager: MemoryManager,
    pub state: TaskState,
    pub vmspace_objects: BTreeMap<VmspaceObjectId, VmspaceObject>,
    pub vmspace_next_id: usize,
    pub cspace: CapabilitySpace,
    pub kernel_channel: UserspaceChannel,
    pub claimed_interrupts: BTreeMap<usize, usize>,
}

impl Task {
    pub fn load<'a, I>(name: &str, elf: &Elf, args: I) -> Self
    where
        I: Iterator<Item = &'a str> + Clone,
    {
        let mut memory_manager = MemoryManager::new();
        let mut cspace = CapabilitySpace::new();

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
                RegionDescription {
                    size: PageSize::Kilopage,
                    len: region_size / 4.kib(),
                    contiguous: false,
                    flags,
                    fill: FillOption::Data(&segment_data[..segment_len]),
                    kind,
                },
            );

            segment_offset = segment_load_base.add(region_size);
        }

        let tls = elf.program_headers().find(|header| header.r#type == elf64::ProgramSegmentType::Tls).map(|header| {
            let n_pages_needed = round_up_to_next(header.memory_size as usize + 8 + 16, 4.kib()) / 4.kib();
            let tls_base = memory_manager.find_free_region(PageSize::Kilopage, n_pages_needed);

            let segment_len = header.file_size as usize + 8 + 16;

            if segment_data.len() < segment_len {
                segment_data.resize(segment_len, 0);
            }

            let segment_file_size = header.file_size as usize;
            let tls_base_addr = tls_base.as_usize();
            segment_data[0..][..8].copy_from_slice(&(tls_base_addr + 8).to_le_bytes()[..]); // ->|
            segment_data[8..][..8].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]); // <-|
            segment_data[16..][..8].copy_from_slice(&(tls_base_addr + 24).to_le_bytes()[..]);

            segment_data[24..][..segment_file_size].copy_from_slice(elf.program_segment_data(&header));
            segment_data[segment_file_size..segment_len].fill(0);

            memory_manager.alloc_region(
                Some(tls_base),
                RegionDescription {
                    size: PageSize::Kilopage,
                    len: n_pages_needed,
                    contiguous: false,
                    flags: USER | READ | WRITE | VALID,
                    fill: FillOption::Data(&segment_data[..segment_len]),
                    kind: AddressRegionKind::Tls,
                },
            );

            tls_base_addr + 24
        });

        // We guard the stack on both ends, though a stack underflow is
        // unlikely, but better to be safe than sorry!
        let sp = memory_manager
            .alloc_guarded_region(RegionDescription {
                size: PageSize::Kilopage,
                len: 16,
                contiguous: false,
                flags: USER | READ | WRITE | VALID,
                fill: FillOption::Unitialized,
                kind: AddressRegionKind::Stack,
            })
            .add(16.kib());

        let fdt_ptr = FDT.load(core::sync::atomic::Ordering::Acquire);
        let fdt_loc = {
            let fdt = unsafe { Fdt::from_ptr(fdt_ptr) }.unwrap();
            let slice = unsafe { core::slice::from_raw_parts(fdt_ptr, fdt.total_size()) };
            memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: PageSize::Kilopage,
                    len: round_up_to_next(fdt.total_size(), 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: USER | READ | VALID,
                    fill: FillOption::Data(slice),
                    kind: AddressRegionKind::Data,
                },
            )
        };

        let arg_count = args.clone().count();
        let (a0, a1) = match arg_count {
            0 => (0, 0),
            n => {
                let total_size = args.clone().fold(0, |total, s| total + s.len());
                let concatenated = args.clone().flat_map(|s| s.bytes()).collect::<Vec<_>>();
                let storage = memory_manager.alloc_guarded_region(RegionDescription {
                    size: PageSize::Kilopage,
                    len: round_up_to_next(total_size, 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: USER | READ | VALID,
                    fill: FillOption::Data(&concatenated),
                    kind: AddressRegionKind::ReadOnly,
                });
                let (_, ptr_list) = args.fold((storage, Vec::new()), |(ptr, mut v), s| {
                    v.extend_from_slice(&ptr.as_usize().to_ne_bytes());
                    v.extend_from_slice(&s.len().to_ne_bytes());

                    (ptr.add(s.len()), v)
                });
                let ptrs = memory_manager.alloc_guarded_region(RegionDescription {
                    size: PageSize::Kilopage,
                    len: round_up_to_next(n * 16, 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: USER | READ | VALID,
                    fill: FillOption::Data(&ptr_list),
                    kind: AddressRegionKind::ReadOnly,
                });

                (n, ptrs.as_usize())
            }
        };

        let context = Context {
            pc: pc.as_usize(),
            gp_regs: GeneralRegisters {
                sp: sp.as_usize(),
                tp: tls.unwrap_or(0),
                a0,
                a1,
                a2: fdt_loc.start.as_usize(),
                ..Default::default()
            },
            fp_regs: FloatingPointRegisters::default(),
        };

        let (kernel_channel, user_read) = UserspaceChannel::new();
        cspace
            .mint_with_id(
                KERNEL_CHANNEL,
                Capability { resource: CapabilityResource::Channel(user_read), rights: CapabilityRights::READ },
            )
            .expect("[BUG] parent channel cap already created?");

        Self {
            tid: Tid::new(NonZeroUsize::new(usize::MAX).unwrap()),
            name: Box::from(name),
            context,
            memory_manager,
            state: TaskState::Running,
            vmspace_objects: BTreeMap::new(),
            vmspace_next_id: 0,
            cspace,
            kernel_channel,
            claimed_interrupts: BTreeMap::new(),
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
