// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::SyscallOutcome;
use crate::{
    capabilities::{Capability, CapabilityResource},
    mem::{
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize},
    },
    task::Task,
    utils,
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::KError,
    message::Message,
    syscalls::allocation::{AllocationOptions, DmaAllocationOptions, MemoryPermissions},
};

pub fn alloc_virtual_memory(
    task: &mut Task,
    size: usize,
    options: AllocationOptions,
    permissions: MemoryPermissions,
) -> SyscallOutcome {
    if permissions & MemoryPermissions::WRITE && !(permissions & MemoryPermissions::READ) {
        return SyscallOutcome::Err(KError::InvalidArgument(2));
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

    let page_size = if options & AllocationOptions::LargePage { PageSize::Megapage } else { PageSize::Kilopage };

    match size {
        0 => SyscallOutcome::Err(KError::InvalidArgument(0)),
        _ => {
            let allocated_at = task.memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: page_size,
                    len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                    contiguous: false,
                    flags,
                    fill: if options & AllocationOptions::Zero { FillOption::Zeroed } else { FillOption::Unitialized },
                    kind: AddressRegionKind::UserAllocated,
                },
            );

            if &*task.name == "init" && page_size == PageSize::Megapage {
                log::info!("Allocated memory at {:#p} ({:?}) for user process", allocated_at.start, page_size);
            }

            SyscallOutcome::Processed(Message::from(allocated_at.start.as_usize()))
        }
    }
}

pub fn alloc_dma_memory(task: &mut Task, size: usize, options: DmaAllocationOptions) -> SyscallOutcome {
    let page_size = PageSize::Kilopage;

    match size {
        0 => SyscallOutcome::Err(KError::InvalidArgument(0)),
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

            SyscallOutcome::processed((phys.as_usize(), allocated_at.start.as_usize()))
        }
    }
}

pub fn query_mem_cap(task: &mut Task, cptr: CapabilityPtr) -> SyscallOutcome {
    match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Memory(_, vmem, _), rights }) => {
            let memory_perms = match (*rights & CapabilityRights::READ, *rights & CapabilityRights::WRITE) {
                (true, true) => MemoryPermissions::READ | MemoryPermissions::WRITE,
                (true, false) => MemoryPermissions::READ,
                _ => unreachable!("[BUG] no memory capabilities should be marked as write-only or non-read-write"),
            };

            SyscallOutcome::processed((
                vmem.start.as_usize(),
                vmem.end.as_usize() - vmem.start.as_usize(),
                memory_perms.value(),
            ))
        }
        _ => SyscallOutcome::Err(KError::InvalidArgument(0)),
    }
}

pub fn query_mmio_cap(task: &mut Task, cptr: CapabilityPtr) -> SyscallOutcome {
    match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Mmio(vmem, interrupts), rights }) => {
            let memory_perms = match (*rights & CapabilityRights::READ, *rights & CapabilityRights::WRITE) {
                (true, true) => MemoryPermissions::READ | MemoryPermissions::WRITE,
                (true, false) => MemoryPermissions::READ,
                _ => unreachable!("[BUG] no memory capabilities should be marked as write-only or non-read-write"),
            };

            if interrupts.len() > 8 {
                log::warn!(
                    "Memory mapped device has more than 8 interrupts! They will be truncated in syscall response!"
                );
            }

            let mut msg = Message::default();
            let n_interrupts = interrupts.len().min(8);
            msg.contents[0] = vmem.start.as_usize();
            msg.contents[1] = vmem.end.as_usize() - vmem.start.as_usize();
            msg.contents[2] = memory_perms.value();
            msg.contents[3] = n_interrupts;
            msg.contents[4..][..n_interrupts].copy_from_slice(&interrupts[..n_interrupts]);

            SyscallOutcome::processed(msg)
        }
        _ => SyscallOutcome::Err(KError::InvalidArgument(0)),
    }
}
