// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource, CapabilitySpace},
    mem::{
        manager::{AddressRegionKind, FillOption, MemoryManager, RegionDescription},
        paging::{flags, PageSize, VirtualAddress},
        user::RawUserSlice,
    },
    scheduler::{Scheduler, CURRENT_TASK, SCHEDULER},
    syscall::channel::UserspaceChannel,
    task::{Context, MessageQueue, Task},
    trap::GeneralRegisters,
    utils::{self, Units},
};
use alloc::vec::Vec;
use librust::{
    capabilities::CapabilityRights,
    error::{AccessError, KError},
    syscalls::{allocation::MemoryPermissions, channel::ChannelId, vmspace::VmspaceObjectId},
};

use super::SyscallOutcome;

pub struct VmspaceObject {
    pub memory_manager: MemoryManager,
    pub inprocess_mappings: Vec<VirtualAddress>,
    pub cspace: CapabilitySpace,
}

impl VmspaceObject {
    pub fn new() -> Self {
        Self { memory_manager: MemoryManager::new(), inprocess_mappings: Vec::new(), cspace: CapabilitySpace::new() }
    }
}

pub fn create_vmspace(task: &mut Task) -> SyscallOutcome {
    let id = task.vmspace_next_id;
    task.vmspace_next_id += 1;
    task.vmspace_objects.insert(VmspaceObjectId::new(id), VmspaceObject::new());

    SyscallOutcome::processed(id)
}

pub fn alloc_vmspace_object(
    task: &mut Task,
    id: usize,
    address: usize,
    size: usize,
    permissions: usize,
) -> SyscallOutcome {
    let object = match task.vmspace_objects.get_mut(&VmspaceObjectId::new(id)) {
        Some(map) => map,
        None => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };

    let permissions = MemoryPermissions::new(permissions);
    let address = VirtualAddress::new(address);

    if !address.is_aligned(PageSize::Kilopage) || address.is_kernel_region() || address.checked_add(size).is_none() {
        return SyscallOutcome::Err(KError::InvalidArgument(1));
    } else if size == 0 {
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

    let kind = match (flags & flags::READ, flags & flags::WRITE, flags & flags::EXECUTE) {
        (true, true, true) => AddressRegionKind::UserAllocated,
        (true, true, false) => AddressRegionKind::Data,
        (true, false, false) => AddressRegionKind::ReadOnly,
        (true, false, true) | (false, false, true) => AddressRegionKind::Text,
        (false, false, false) | (false, true, true) | (false, true, false) => {
            return SyscallOutcome::Err(KError::InvalidArgument(3))
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

    SyscallOutcome::processed((range.start.as_usize(), at.start.as_usize()))
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_vmspace(
    task: &mut Task,
    id: VmspaceObjectId,
    name: VirtualAddress,
    len: usize,
    pc: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    sp: usize,
    tp: usize,
) -> SyscallOutcome {
    let current_tid = CURRENT_TASK.get().unwrap();

    let object = match task.vmspace_objects.remove(&id) {
        Some(map) => map,
        None => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };

    let user_slice = RawUserSlice::readable(name, len);
    let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
        Ok(slice) => slice,
        Err((addr, e)) => {
            log::error!("Bad memory from process: {:?}", e);
            return SyscallOutcome::Err(KError::InvalidAccess(AccessError::Read(addr.as_ptr())));
        }
    };

    let slice = user_slice.guarded();
    let task_name = match core::str::from_utf8(&slice) {
        Ok(s) => s,
        Err(_) => {
            log::error!("Invalid UTF-8 in FDT node name from process");
            return SyscallOutcome::Err(KError::InvalidArgument(1));
        }
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

    let mut new_task = Task {
        name: alloc::string::String::from(task_name).into_boxed_str(),
        context: Context {
            pc,
            gp_regs: GeneralRegisters { a0, a1, a2, sp, tp, ..Default::default() },
            fp_regs: Default::default(),
        },
        memory_manager: object.memory_manager,
        state: crate::task::TaskState::Running,
        message_queue: MessageQueue::new(),
        promiscuous: true,
        incoming_channel_request: Default::default(),
        channels: Default::default(),
        vmspace_next_id: 0,
        vmspace_objects: Default::default(),
        cspace: CapabilitySpace::new(),
    };

    let this_new_channel_id = ChannelId::new(task.channels.last_key_value().map(|(id, _)| id.value() + 1).unwrap_or(0));
    let (channel1, channel2) = UserspaceChannel::new();
    new_task.channels.insert(ChannelId::new(0), (current_tid, channel1));
    new_task.cspace.mint(Capability {
        resource: CapabilityResource::Channel(ChannelId::new(0)),
        rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
    });

    for region in object.inprocess_mappings {
        task.memory_manager.dealloc_region(region);
    }

    // FIXME: this is gross, should have a way to reserve a TID (or ideally not
    // need it at all) so we don't have to lock the task after insertion

    let tid = SCHEDULER.enqueue(new_task);

    task.channels.insert(this_new_channel_id, (tid, channel2));
    let cptr = task.cspace.mint(Capability {
        resource: CapabilityResource::Channel(this_new_channel_id),
        rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
    });

    SyscallOutcome::processed((tid.value(), cptr.value()))
}
