// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource, MmioRegion, SharedMemory},
    mem::{
        manager::{AddressRegion, AddressRegionKind, FillOption, RegionDescription},
        paging::{flags::Flags, PageSize, VirtualAddress},
        user::{RawUserSlice, ReadWrite, ValidatedUserSlice},
    },
    task::Task,
    trap::GeneralRegisters,
    utils::{self, Units},
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::mem::{DmaAllocationOptions, MemoryPermissions},
};

pub fn allocate_shared_memory(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();

    let size = frame.a1;
    let permissions = MemoryPermissions::new(frame.a2);

    if permissions & MemoryPermissions::WRITE && !(permissions & MemoryPermissions::READ) {
        return Err(SyscallError::InvalidArgument(1));
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

    let page_size = if size >= 2.mib() { PageSize::Megapage } else { PageSize::Kilopage };

    match size {
        0 => Err(SyscallError::InvalidArgument(0)),
        _ => {
            let (allocated_at, region) = task.memory_manager.alloc_shared_region(
                None,
                RegionDescription {
                    size: page_size,
                    count: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                    contiguous: false,
                    flags,
                    fill: FillOption::Zeroed,
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
                resource: CapabilityResource::SharedMemory(SharedMemory {
                    physical_region: region,
                    virtual_range: allocated_at.clone(),
                    kind: AddressRegionKind::UserSharedMemory,
                }),
                rights: rights | CapabilityRights::GRANT,
            });

            log::trace!("Allocated shared memory at {:#p} ({:?}) for user process", allocated_at.start, page_size);

            frame.a1 = cptr.value();
            frame.a2 = allocated_at.start.as_usize();
            frame.a3 = allocated_at.end.as_usize() - allocated_at.start.as_usize();

            Ok(())
        }
    }
}

pub fn allocate_virtual_memory(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();

    let size = frame.a1;
    let permissions = MemoryPermissions::new(frame.a2);

    if permissions & MemoryPermissions::WRITE && !(permissions & MemoryPermissions::READ) {
        return Err(SyscallError::InvalidArgument(1));
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

    let page_size = if size >= 2.mib() { PageSize::Megapage } else { PageSize::Kilopage };

    match size {
        0 => Err(SyscallError::InvalidArgument(0)),
        _ => {
            let allocated_at = task.memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: page_size,
                    count: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                    contiguous: false,
                    flags,
                    fill: FillOption::Zeroed,
                    kind: AddressRegionKind::UserAllocated,
                },
            );

            log::trace!("Allocated memory at {:#p} ({:?}) for user process", allocated_at.start, page_size);

            frame.a1 = allocated_at.start.as_usize();
            frame.a2 = allocated_at.end.as_usize() - allocated_at.start.as_usize();

            Ok(())
        }
    }
}

pub fn deallocate_virtual_memory(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let address = VirtualAddress::new(frame.a1);

    match task.memory_manager.region_for(address) {
        Some(AddressRegion { kind: AddressRegionKind::UserAllocated, .. }) => {}
        Some(_) | None => return Err(SyscallError::InvalidArgument(0)),
    }

    task.memory_manager.dealloc_region(address);

    Ok(())
}

pub fn allocate_device_addressable_memory(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let size = frame.a1;
    let options = DmaAllocationOptions::new(frame.a2);
    let page_size = PageSize::Kilopage;
    let mut task = task.mutable_state.lock();

    match size {
        0 => Err(SyscallError::InvalidArgument(0)),
        _ => {
            let allocated_at = task.memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: page_size,
                    count: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
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

pub fn query_mem_cap(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);

    match task.mutable_state.lock().cspace.resolve(cptr) {
        Some(Capability {
            resource: CapabilityResource::SharedMemory(SharedMemory { virtual_range: vmem, .. }),
            rights,
        }) => {
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

pub fn query_mmio_cap(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let task = task.mutable_state.lock();

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
        Some(Capability {
            resource: CapabilityResource::Mmio(MmioRegion { interrupts, virtual_range: vmem, .. }),
            ..
        }) => {
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
