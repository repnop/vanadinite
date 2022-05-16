// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource},
    mem::{
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize, VirtualAddress}, user::{RawUserSlice, ValidatedUserSlice, ReadWrite},
    },
    task::Task,
    utils, trap::TrapFrame,
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    syscalls::mem::{AllocationOptions, DmaAllocationOptions, MemoryPermissions}, error::SyscallError,
};

pub fn alloc_virtual_memory(
    task: &mut Task,
    frame: &mut TrapFrame,
) -> Result<(), SyscallError> {
    let size = frame.a1;
    let options = AllocationOptions::new(frame.a2);
    let permissions = MemoryPermissions::new(frame.a3);

    if permissions & MemoryPermissions::WRITE && !(permissions & MemoryPermissions::READ) {
        return Err(SyscallError::InvalidArgument(2));
    }

    let mut flags = flags::VALID | flags::USER;

    if permissions & MemoryPermissions::READ {
        flags |= flags::READ;
    }

    if permissions & MemoryPermissions::WRITE {
        flags |= flags::WRITE;
    }

    if permissions & MemoryPermissions::EXECUTE {
        flags |= flags::EXECUTE;
    }

    let page_size = if options & AllocationOptions::LARGE_PAGE { PageSize::Megapage } else { PageSize::Kilopage };

    match size {
        0 => Err(SyscallError::InvalidArgument(0)),
        _ => {
            let allocated_at = task.memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: page_size,
                    len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                    contiguous: false,
                    flags,
                    fill: if options & AllocationOptions::ZERO { FillOption::Zeroed } else { FillOption::Unitialized },
                    kind: AddressRegionKind::UserAllocated,
                },
            );

            log::trace!("Allocated memory at {:#p} ({:?}) for user process", allocated_at.start, page_size);

            frame.a1 = allocated_at.start.as_usize();
            Ok(())
        }
    }
}

pub fn alloc_dma_memory(task: &mut Task, frame: &mut TrapFrame) -> Result<(), SyscallError> {
    let size = frame.a1;
    let options = DmaAllocationOptions::new(frame.a2);
    let page_size = PageSize::Kilopage;

    match size {
        0 => Err(SyscallError::InvalidArgument(0)),
        _ => {
            let allocated_at = task.memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: page_size,
                    len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                    contiguous: true,
                    flags: flags::VALID | flags::USER | flags::READ | flags::WRITE,
                    fill: if options & DmaAllocationOptions::ZERO {
                        FillOption::Zeroed
                    } else {
                        FillOption::Unitialized
                    },
                    kind: AddressRegionKind::Dma,
                },
            );

            let phys = task.memory_manager.resolve(allocated_at.start).unwrap();

            log::debug!("Allocated DMA memory at {:#p} for user process", allocated_at.start);

            frame.a1 = phys.as_usize();
            frame.a2 = allocated_at.start.as_usize();
            Ok(())
        }
    }
}

pub fn query_mem_cap(task: &mut Task, frame: &mut TrapFrame) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);

    match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Memory(_, vmem, _), rights }) => {
            let memory_perms = match (*rights & CapabilityRights::READ, *rights & CapabilityRights::WRITE) {
                (true, true) => MemoryPermissions::READ | MemoryPermissions::WRITE,
                (true, false) => MemoryPermissions::READ,
                _ => unreachable!("[BUG] no memory capabilities should be marked as write-only or non-read-write"),
            };
            
            frame.a1 = vmem.start.as_usize();
            frame.a2 = vmem.end.as_usize() - vmem.start.as_usize();
            frame.a3 = memory_perms.value();
            Ok(())
        }
        _ => Err(SyscallError::InvalidArgument(0)),
    }
}

pub fn query_mmio_cap(task: &mut Task, frame: &mut TrapFrame) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);
    let buffer_ptr = VirtualAddress::new(frame.a2);
    let buffer_len = frame.a3;
    let buffer: ValidatedUserSlice<ReadWrite, usize> = match unsafe { RawUserSlice::new(buffer_ptr, buffer_len).validate(&task.memory_manager) } {
        Ok(slice) => slice,
        Err((_, e)) => {
            log::debug!("Bad interrupt buffer @ {:#p}: {}", buffer_ptr, e);
            return Err(SyscallError::InvalidArgument(1));
        }
    };

    match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Mmio(vmem, interrupts), rights }) => {
            let memory_perms = match (*rights & CapabilityRights::READ, *rights & CapabilityRights::WRITE) {
                (true, true) => MemoryPermissions::READ | MemoryPermissions::WRITE,
                (true, false) => MemoryPermissions::READ,
                _ => unreachable!("[BUG] no memory capabilities should be marked as write-only or non-read-write"),
            };

            let n_interrupts = interrupts.len();

            let write_n = buffer.len().min(n_interrupts);
            buffer.guarded()[..write_n].copy_from_slice(&interrupts[..write_n]);

            frame.a1 = vmem.start.as_usize();
            frame.a2 = vmem.end.as_usize() - vmem.start.as_usize();
            frame.a3 = memory_perms.value();
            frame.a4 = n_interrupts;
            frame.a5 = write_n;

            Ok(())
        }
        _ => Err(SyscallError::InvalidArgument(0)),
    }
}
