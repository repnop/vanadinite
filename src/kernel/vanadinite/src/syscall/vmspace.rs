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
        manager::{AddressRegionKind, FillOption, RegionDescription, UserspaceMemoryManager},
        paging::{flags::Flags, PageSize, VirtualAddress},
        user::RawUserSlice,
    },
    scheduler::{Scheduler, SCHEDULER},
    syscall::channel::UserspaceChannel,
    task::{Context, Task},
    trap::GeneralRegisters,
    utils::{self, Units},
};
use alloc::{collections::BTreeMap, vec::Vec};
use librust::{
    capabilities::CapabilityRights,
    error::SyscallError,
    syscalls::{
        channel::{KERNEL_CHANNEL, PARENT_CHANNEL},
        mem::MemoryPermissions,
        vmspace::VmspaceObjectId,
    },
    task::Tid,
};

pub struct VmspaceObject {
    pub memory_manager: UserspaceMemoryManager,
    pub inprocess_mappings: Vec<VirtualAddress>,
    pub cspace: CapabilitySpace,
}

impl VmspaceObject {
    pub fn new() -> Self {
        Self {
            memory_manager: UserspaceMemoryManager::new(),
            inprocess_mappings: Vec::new(),
            cspace: CapabilitySpace::new(),
        }
    }
}

pub fn create_vmspace(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let id = task.vmspace_next_id;
    task.vmspace_next_id += 1;
    task.vmspace_objects.insert(VmspaceObjectId::new(id), VmspaceObject::new());

    frame.a1 = id;
    Ok(())
}

pub fn alloc_vmspace_object(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let id = frame.a1;
    let address = VirtualAddress::new(frame.a2);
    let size = frame.a3;
    let permissions = MemoryPermissions::new(frame.a4);

    let object = match task.vmspace_objects.get_mut(&VmspaceObjectId::new(id)) {
        Some(map) => map,
        None => return Err(SyscallError::InvalidArgument(0)),
    };

    if !address.is_aligned(PageSize::Kilopage) || address.is_kernel_region() || address.checked_add(size).is_none() {
        return Err(SyscallError::InvalidArgument(1));
    } else if size == 0 {
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

    let kind = match (flags & Flags::READ, flags & Flags::WRITE, flags & Flags::EXECUTE) {
        (true, true, true) => AddressRegionKind::UserAllocated,
        (true, true, false) => AddressRegionKind::Data,
        (true, false, false) => AddressRegionKind::ReadOnly,
        (true, false, true) | (false, false, true) => AddressRegionKind::Text,
        (false, false, false) | (false, true, true) | (false, true, false) => {
            return Err(SyscallError::InvalidArgument(3))
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

    // log::info!("Mapping region at {:#p} for task {}", at.start, task.name);
    let range = task.memory_manager.apply_shared_region(
        None,
        Flags::USER | Flags::VALID | Flags::READ | Flags::WRITE,
        region,
        AddressRegionKind::UserAllocated,
    );

    object.inprocess_mappings.push(range.start);
    log::debug!("added {:#p} to task vmspace", range.start);

    frame.a1 = range.start.as_usize();
    frame.a2 = at.start.as_usize();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_vmspace(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let id: VmspaceObjectId = VmspaceObjectId::new(frame.a1);
    let name: VirtualAddress = VirtualAddress::new(frame.a2);
    let len: usize = frame.a3;
    let pc: usize = frame.t0;
    let a0: usize = frame.t1;
    let a1: usize = frame.t2;
    let a2: usize = frame.t3;
    let sp: usize = frame.t4;
    let tp: usize = frame.t5;

    let object = match task.vmspace_objects.remove(&id) {
        Some(map) => map,
        None => return Err(SyscallError::InvalidArgument(0)),
    };

    let user_slice = RawUserSlice::readable(name, len);
    let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
        Ok(slice) => slice,
        Err((_, e)) => {
            log::error!("Bad memory from process: {:?}", e);
            return Err(SyscallError::InvalidArgument(1));
        }
    };

    let slice = user_slice.guarded();
    let task_name = match core::str::from_utf8(&slice) {
        Ok(s) => s,
        Err(_) => {
            log::error!("Invalid UTF-8 in FDT node name from process");
            return Err(SyscallError::InvalidArgument(1));
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
    log::debug!("Memory map:\n{:#?}", object.memory_manager.address_map_debug(None));

    let (kernel_channel, user_read) = UserspaceChannel::new();
    let mut new_task = Task {
        tid: Tid::new(NonZeroUsize::new(usize::MAX).unwrap()),
        name: alloc::string::String::from(task_name).into_boxed_str(),
        context: Context {
            pc,
            gp_regs: GeneralRegisters { a0, a1, a2, sp, tp, ..Default::default() },
            fp_regs: Default::default(),
        },
        memory_manager: object.memory_manager,
        state: crate::task::TaskState::Running,
        vmspace_next_id: 0,
        vmspace_objects: Default::default(),
        cspace: CapabilitySpace::new(),
        kernel_channel,
        claimed_interrupts: BTreeMap::new(),
        subscribes_to_events: true,
    };

    let (mut channel1, mut channel2) = UserspaceChannel::new();

    new_task
        .cspace
        .mint_with_id(
            KERNEL_CHANNEL,
            Capability { resource: CapabilityResource::Channel(user_read), rights: CapabilityRights::READ },
        )
        .expect("[BUG] kernel channel cap already created?");

    SCHEDULER.enqueue_with(|tid| {
        channel1.sender.other_tid = Some(tid);
        channel2.sender.other_tid = Some(task.tid);

        for region in object.inprocess_mappings {
            task.memory_manager.dealloc_region(region);
        }

        let cptr = task.cspace.mint_with(|cptr| {
            channel1.sender.other_cptr = PARENT_CHANNEL;
            channel2.sender.other_cptr = cptr;

            new_task
                .cspace
                .mint_with_id(
                    PARENT_CHANNEL,
                    Capability {
                        resource: CapabilityResource::Channel(channel2),
                        rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
                    },
                )
                .expect("[BUG] parent channel cap already created?");

            Capability {
                resource: CapabilityResource::Channel(channel1),
                rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
            }
        });

        frame.a1 = cptr.value();

        new_task
    });

    Ok(())
}
