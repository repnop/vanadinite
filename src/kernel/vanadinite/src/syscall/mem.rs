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
        paging::{flags::Flags, PageSize, VirtualAddress},
        user::{RawUserSlice, ReadWrite, ValidatedUserSlice},
    },
    task::Task,
    trap::GeneralRegisters,
    utils,
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::mem::{AllocationOptions, DmaAllocationOptions, MemoryPermissions},
};

pub fn alloc_virtual_memory(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let size = frame.a1;
    let options = AllocationOptions::new(frame.a2);
    let permissions = MemoryPermissions::new(frame.a3);

    if permissions & MemoryPermissions::WRITE && !(permissions & MemoryPermissions::READ) {
        return Err(SyscallError::InvalidArgument(2));
    }

    let mut flags = Flags::VALID | Flags::USER;

    if permissions & MemoryPermissions::READ {
        flags |= Flags::READ;
    }

    if permissions & MemoryPermissions::WRITE {
        flags |= Flags::WRITE;
    }

    if permissions & MemoryPermissions::EXECUTE {
        flags |= Flags::EXECUTE;
    }

    let page_size = if options & AllocationOptions::LARGE_PAGE { PageSize::Megapage } else { PageSize::Kilopage };

    match size {
        0 => Err(SyscallError::InvalidArgument(0)),
        _ => {
            let (cptr, allocated_at) = if options & AllocationOptions::PRIVATE {
                let allocated_at = task.memory_manager.alloc_region(
                    None,
                    RegionDescription {
                        size: page_size,
                        len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                        contiguous: false,
                        flags,
                        fill: if options & AllocationOptions::ZERO {
                            FillOption::Zeroed
                        } else {
                            FillOption::Unitialized
                        },
                        kind: AddressRegionKind::UserAllocated,
                    },
                );

                (CapabilityPtr::new(usize::MAX), allocated_at)
            } else {
                let (allocated_at, region) = task.memory_manager.alloc_shared_region(
                    None,
                    RegionDescription {
                        size: page_size,
                        len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                        contiguous: false,
                        flags,
                        fill: if options & AllocationOptions::ZERO {
                            FillOption::Zeroed
                        } else {
                            FillOption::Unitialized
                        },
                        kind: AddressRegionKind::UserAllocated,
                    },
                );

                let rights = match (
                    permissions & MemoryPermissions::READ,
                    permissions & MemoryPermissions::WRITE,
                    permissions & MemoryPermissions::EXECUTE,
                ) {
                    (true, true, true) => CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::EXECUTE,
                    (true, true, false) => CapabilityRights::READ | CapabilityRights::WRITE,
                    (true, false, false) => CapabilityRights::READ,
                    (r, w, x) => unreachable!("read={r} write={w} execute={x}"),
                };

                let cptr = task.cspace.mint(Capability {
                    resource: CapabilityResource::Memory(
                        region,
                        allocated_at.clone(),
                        AddressRegionKind::UserAllocated,
                    ),
                    rights: rights | CapabilityRights::GRANT,
                });

                (cptr, allocated_at)
            };

            log::trace!("Allocated memory at {:#p} ({:?}) for user process", allocated_at.start, page_size);

            frame.a1 = cptr.value();
            frame.a2 = allocated_at.start.as_usize();
            frame.a3 = allocated_at.end.as_usize() - allocated_at.start.as_usize();

            Ok(())
        }
    }
}

pub fn alloc_dma_memory(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
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
                    flags: Flags::VALID | Flags::USER | Flags::READ | Flags::WRITE,
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

pub fn query_mem_cap(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
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

pub fn query_mmio_cap(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);
    let buffer_ptr = VirtualAddress::new(frame.a2);
    let buffer_len = frame.a3;
    let buffer: ValidatedUserSlice<ReadWrite, usize> =
        match unsafe { RawUserSlice::new(buffer_ptr, buffer_len).validate(&task.memory_manager) } {
            Ok(slice) => slice,
            Err((_, e)) => {
                log::debug!("Bad interrupt buffer @ {:#p}: {:?}", buffer_ptr, e);
                return Err(SyscallError::InvalidArgument(1));
            }
        };

    match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Mmio(_, vmem, interrupts), .. }) => {
            let n_interrupts = interrupts.len();

            let write_n = buffer.len().min(n_interrupts);
            buffer.guarded()[..write_n].copy_from_slice(&interrupts[..write_n]);

            frame.a1 = vmem.start.as_usize();
            frame.a2 = vmem.end.as_usize() - vmem.start.as_usize();
            frame.a3 = n_interrupts;
            frame.a4 = write_n;

            Ok(())
        }
        _ => Err(SyscallError::InvalidArgument(0)),
    }
}
