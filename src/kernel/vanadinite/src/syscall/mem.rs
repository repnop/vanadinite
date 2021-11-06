// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::SyscallOutcome;
use crate::{
    mem::{
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize},
    },
    task::Task,
    utils,
};
use librust::{
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

            log::debug!("Allocated memory at {:#p} for user process", allocated_at.start);

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
