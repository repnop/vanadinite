// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::AtomicUsize;

use crate::{
    capabilities::{Capability, CapabilityResource, CapabilityRights, CapabilitySpace},
    mem::{
        manager::{AddressRegionKind, FillOption, MemoryManager, RegionDescription},
        paging::{flags, PageSize, VirtualAddress},
        user::RawUserSlice,
    },
    scheduler::{Scheduler, CURRENT_TASK, SCHEDULER, TASKS},
    syscall::channel::UserspaceChannel,
    task::{Context, Task},
    trap::GeneralRegisters,
    utils::{self, Units},
};
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use librust::{
    capabilities::CapabilityPtr,
    error::KError,
    message::{KernelNotification, Message, Sender, SyscallResult},
    syscalls::{allocation::MemoryPermissions, channel::ChannelId, vmspace::VmspaceObjectId},
};

pub struct VmspaceObject {
    pub memory_manager: MemoryManager,
    pub inprocess_mappings: Vec<VirtualAddress>,
    pub cspace: CapabilitySpace,
    pub service_name_to_cptr: BTreeMap<alloc::string::String, (CapabilityPtr, CapabilityRights)>,
}

impl VmspaceObject {
    pub fn new() -> Self {
        Self {
            memory_manager: MemoryManager::new(),
            inprocess_mappings: Vec::new(),
            cspace: CapabilitySpace::new(),
            service_name_to_cptr: BTreeMap::new(),
        }
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
) -> SyscallResult<(usize, usize), KError> {
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

    let mut new_task = Task {
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

    let this_new_channel_id = ChannelId::new(task.channels.last_key_value().map(|(id, _)| id.value() + 1).unwrap_or(0));
    let msg_counter = Arc::new(AtomicUsize::new(0));
    new_task.channels.insert(
        ChannelId::new(0),
        UserspaceChannel::new(CURRENT_TASK.get().unwrap(), this_new_channel_id, Arc::clone(&msg_counter)),
    );
    new_task.cspace.mint(Capability {
        resource: CapabilityResource::Channel(ChannelId::new(0)),
        rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
    });

    for region in object.inprocess_mappings {
        task.memory_manager.dealloc_region(region);
    }

    let tid = SCHEDULER.enqueue(new_task);

    let channel = UserspaceChannel::new(tid, ChannelId::new(0), msg_counter);
    task.channels.insert(this_new_channel_id, channel);
    let cptr = task.cspace.mint(Capability {
        resource: CapabilityResource::Channel(this_new_channel_id),
        rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
    });

    // FIXME: this is gross, should have a way to reserve a TID (or ideally not
    // need it at all) so we don't have to lock the task after insertion

    let new_task = TASKS.get(tid).unwrap();
    let mut new_task = new_task.lock();

    for (name, (cptr, rights)) in object.service_name_to_cptr {
        let cap = match task.cspace.resolve(cptr) {
            Some(cap) => cap,
            None => return SyscallResult::Err(KError::InvalidArgument(1)),
        };

        match &cap.resource {
            CapabilityResource::Channel(channel_id) => {
                // FIXME: can this unwrap fail..?
                let channel = task.channels.get(channel_id).unwrap();
                let other_task = match TASKS.get(channel.other_task) {
                    Some(task) => task,
                    None => panic!("wut"),
                };

                let mut other_task = other_task.lock();
                if other_task.state.is_dead() {
                    // FIXME: report error or?
                    continue;
                }

                let other_rights = other_task
                    .cspace
                    .all()
                    .find_map(|(_, cap)| match cap {
                        Capability { resource: CapabilityResource::Channel(id), rights } => {
                            match other_task.channels.get(id).unwrap().other_channel_id == *channel_id {
                                true => Some(*rights),
                                false => None,
                            }
                        }
                    })
                    .unwrap();

                let new_task_channel_id =
                    ChannelId::new(new_task.channels.last_key_value().map(|(id, _)| id.value() + 1).unwrap_or(0));
                let other_task_channel_id =
                    ChannelId::new(other_task.channels.last_key_value().map(|(id, _)| id.value() + 1).unwrap_or(0));
                let msg_counter = Arc::new(AtomicUsize::new(0));

                new_task.channels.insert(
                    new_task_channel_id,
                    UserspaceChannel::new(channel.other_task, other_task_channel_id, Arc::clone(&msg_counter)),
                );
                other_task
                    .channels
                    .insert(other_task_channel_id, UserspaceChannel::new(tid, new_task_channel_id, msg_counter));

                let cptr = new_task
                    .cspace
                    .mint(Capability { resource: CapabilityResource::Channel(new_task_channel_id), rights });
                let cptr = other_task.cspace.mint(Capability {
                    resource: CapabilityResource::Channel(other_task_channel_id),
                    rights: other_rights,
                });

                new_task
                    .message_queue
                    .push_back((Sender::kernel(), Message::from(KernelNotification::ChannelOpened(cptr))));
            }
        }
    }

    SyscallResult::Ok((tid.value(), cptr.value()))
}

pub fn grant_capability(
    task: &mut Task,
    id: usize,
    cptr: CapabilityPtr,
    name: *const u8,
    len: usize,
    rights: CapabilityRights,
) -> SyscallResult<()> {
    let vmspace = match task.vmspace_objects.get_mut(&VmspaceObjectId::new(id)) {
        Some(vmspace) => vmspace,
        None => return SyscallResult::Err(KError::InvalidArgument(0)),
    };

    let cap = match task.cspace.resolve(cptr) {
        Some(cap) => cap,
        None => return SyscallResult::Err(KError::InvalidArgument(1)),
    };

    if !cap.rights.is_superset(rights) {
        return SyscallResult::Err(KError::InvalidArgument(4));
    }

    let name =
        match unsafe { RawUserSlice::readable(VirtualAddress::from_ptr(name), len).validate(&task.memory_manager) } {
            Ok(slice) => slice,
            Err(_) => return SyscallResult::Err(KError::InvalidArgument(2)),
        };

    let name = name.guarded();
    let name = match core::str::from_utf8(&name) {
        Ok(name) => name,
        Err(_) => return SyscallResult::Err(KError::InvalidArgument(2)),
    };

    vmspace.service_name_to_cptr.insert(name.into(), (cptr, rights));

    SyscallResult::Ok(())
}
