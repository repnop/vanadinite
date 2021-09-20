// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::CapabilitySpace,
    mem::{
        manager::{AddressRegionKind, FillOption, MemoryManager, RegionDescription},
        paging::{flags, PageSize, VirtualAddress},
    },
    scheduler::{Scheduler, CURRENT_TASK, SCHEDULER},
    task::{Context, Task},
    trap::GeneralRegisters,
    utils::{self, Units},
};
use alloc::vec::Vec;
use librust::{
    error::KError,
    message::SyscallResult,
    syscalls::{allocation::MemoryPermissions, vmspace::VmspaceObjectId},
};

pub struct VmspaceObject {
    pub memory_manager: MemoryManager,
    pub inprocess_mappings: Vec<VirtualAddress>,
}

impl VmspaceObject {
    pub fn new() -> Self {
        Self { memory_manager: MemoryManager::new(), inprocess_mappings: Vec::new() }
    }
}

pub fn create_vmspace(task: &mut Task) -> SyscallResult<usize, KError> {
    let id = task.vmspace_next_id;
    task.vmspace_next_id += 1;
    task.vmspace_objects.insert(VmspaceObjectId::new(id), VmspaceObject::new());

    SyscallResult::Ok(id)
}

pub fn alloc_vmspace_object(
    task: &mut Task,
    id: usize,
    address: usize,
    size: usize,
    permissions: usize,
) -> SyscallResult<(usize, usize), KError> {
    let object = match task.vmspace_objects.get_mut(&VmspaceObjectId::new(id)) {
        Some(map) => map,
        None => return SyscallResult::Err(KError::InvalidArgument(0)),
    };

    let permissions = MemoryPermissions::new(permissions);
    let address = VirtualAddress::new(address);

    if !address.is_aligned(PageSize::Kilopage) || address.is_kernel_region() || address.checked_add(size).is_none() {
        return SyscallResult::Err(KError::InvalidArgument(1));
    } else if size == 0 {
        return SyscallResult::Err(KError::InvalidArgument(2));
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

    let kind = match (flags & flags::READ, flags & flags::WRITE, flags & flags::EXECUTE) {
        (true, true, true) => AddressRegionKind::UserAllocated,
        (true, true, false) => AddressRegionKind::Data,
        (true, false, false) => AddressRegionKind::ReadOnly,
        (true, false, true) | (false, false, true) => AddressRegionKind::Text,
        (false, false, false) | (false, true, true) | (false, true, false) => {
            return SyscallResult::Err(KError::InvalidArgument(3))
        }
    };

    let size = utils::round_up_to_next(size, 4.kib());
    let at = match address.is_null() {
        true => None,
        false => Some(address),
    };

    let (at, region) = object.memory_manager.alloc_shared_region(
        at,
        RegionDescription {
            size: PageSize::Kilopage,
            len: size / 4.kib(),
            contiguous: false,
            flags,
            fill: FillOption::Zeroed,
            kind,
        },
    );

    let range = task.memory_manager.apply_shared_region(
        None,
        flags::USER | flags::VALID | flags::READ | flags::WRITE,
        region,
        AddressRegionKind::UserAllocated,
    );

    object.inprocess_mappings.push(range.start);
    log::debug!("added {:#p} to task vmspace", range.start);

    SyscallResult::Ok((range.start.as_usize(), at.start.as_usize()))
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_vmspace(
    task: &mut Task,
    id: usize,
    pc: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    sp: usize,
    tp: usize,
) -> SyscallResult<usize, KError> {
    let object = match task.vmspace_objects.remove(&VmspaceObjectId::new(id)) {
        Some(map) => map,
        None => return SyscallResult::Err(KError::InvalidArgument(0)),
    };

    log::debug!(
        "Spawning new task: pc={:#p} sp={:#p} tp={:#p} a0={:x} a1={:x} a2={:x}",
        pc as *const u8,
        sp as *const u8,
        tp as *const u8,
        a0,
        a1,
        a2
    );
    log::debug!("Memory map:\n{:#?}", object.memory_manager.address_map_debug());

    let new_task = Task {
        name: alloc::format!("userspace allocated task by {:?}", CURRENT_TASK.get().unwrap()).into_boxed_str(),
        context: Context {
            pc,
            gp_regs: GeneralRegisters { a0, a1, a2, sp, tp, ..Default::default() },
            fp_regs: Default::default(),
        },
        memory_manager: object.memory_manager,
        state: crate::task::TaskState::Running,
        message_queue: Default::default(),
        promiscuous: true,
        incoming_channel_request: Default::default(),
        channels: Default::default(),
        vmspace_next_id: 0,
        vmspace_objects: Default::default(),
        cspace: CapabilitySpace::new(),
    };

    for region in object.inprocess_mappings {
        task.memory_manager.dealloc_region(region);
    }

    let tid = SCHEDULER.enqueue(new_task);

    SyscallResult::Ok(tid.value())
}
