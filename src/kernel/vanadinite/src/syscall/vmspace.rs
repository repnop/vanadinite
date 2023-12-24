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
        alloc_kernel_stack,
        manager::{AddressRegionKind, FillOption, RegionDescription, UserspaceMemoryManager},
        paging::{flags::Flags, PageSize, VirtualAddress},
        user::RawUserSlice,
    },
    scheduler::{return_to_usermode, SCHEDULER},
    sync::{NoCheck, SpinMutex},
    syscall::endpoint::ChannelEndpoint,
    task::{Context, MutableState, Task, TaskState},
    trap::{GeneralRegisters, TrapFrame},
    utils::{self, Counter, SameHartDeadlockDetection, Units},
};
use alloc::{collections::BTreeMap, vec::Vec};
use librust::{
    capabilities::CapabilityRights,
    error::SyscallError,
    syscalls::{
        endpoint::{OWN_ENDPOINT, PARENT_CHANNEL},
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

pub fn create_vmspace(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let id = task.vmspace_next_id.increment() as usize;
    task.vmspace_objects.insert(VmspaceObjectId::new(id), VmspaceObject::new());

    frame.a1 = id;
    Ok(())
}

pub fn alloc_vmspace_object(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let MutableState { memory_manager, vmspace_objects, .. } = &mut *task;

    let id = frame.a1;
    let address = VirtualAddress::new(frame.a2);
    let size = frame.a3;
    let permissions = MemoryPermissions::new(frame.a4);

    let object = match vmspace_objects.get_mut(&VmspaceObjectId::new(id)) {
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
            count: size / 4.kib(),
            contiguous: false,
            flags,
            fill: FillOption::Zeroed,
            kind,
        },
    );

    // log::info!("Mapping region at {:#p} for task {}", at.start, task.name);
    let range = memory_manager.apply_shared_region(
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
pub fn spawn_vmspace(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task_state = task.mutable_state.lock();

    let id: VmspaceObjectId = VmspaceObjectId::new(frame.a1);
    let name: VirtualAddress = VirtualAddress::new(frame.a2);
    let len: usize = frame.a3;
    let pc: usize = frame.t0;
    let a0: usize = frame.t1;
    let a1: usize = frame.t2;
    let a2: usize = frame.t3;
    let sp: usize = frame.t4;
    let tp: usize = frame.t5;

    let object = match task_state.vmspace_objects.remove(&id) {
        Some(map) => map,
        None => return Err(SyscallError::InvalidArgument(0)),
    };

    let user_slice = RawUserSlice::readable(name, len);
    let user_slice = match unsafe { user_slice.validate(&task_state.memory_manager) } {
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

    let kernel_stack = alloc_kernel_stack(2.mib());
    let trap_frame = unsafe { kernel_stack.sub(core::mem::size_of::<TrapFrame>()).cast::<TrapFrame>() };
    unsafe {
        *trap_frame = TrapFrame { sepc: pc, registers: GeneralRegisters { sp, tp, a0, a1, a2, ..Default::default() } }
    };

    let endpoint = ChannelEndpoint::new();
    let mut cspace = CapabilitySpace::new();
    cspace
        .mint_with_id(
            OWN_ENDPOINT,
            Capability { resource: CapabilityResource::Channel(endpoint.clone()), rights: CapabilityRights::READ },
        )
        .unwrap();

    let mut new_task = Task {
        tid: Tid::new(NonZeroUsize::new(usize::MAX).unwrap()),
        name: alloc::string::String::from(task_name).into_boxed_str(),
        context: SpinMutex::new(
            Context {
                ra: return_to_usermode as usize,
                sp: kernel_stack.addr() - core::mem::size_of::<TrapFrame>(),
                sx: [0; 12],
            },
            NoCheck,
        ),
        kernel_stack,
        endpoint: endpoint.clone(),
        mutable_state: SpinMutex::new(
            MutableState {
                memory_manager: object.memory_manager,
                vmspace_next_id: Counter::new(),
                reply_next_id: Counter::new(),
                vmspace_objects: Default::default(),
                cspace,
                lock_regions: BTreeMap::new(),
                claimed_interrupts: BTreeMap::new(),
                subscribes_to_events: true,
                state: TaskState::Ready,
            },
            SameHartDeadlockDetection::new(),
        ),
    };

    let parent_endpoint = task.endpoint.clone();

    new_task
        .mutable_state
        .get_mut()
        .cspace
        .mint_with_id(
            OWN_ENDPOINT,
            Capability { resource: CapabilityResource::Channel(endpoint.clone()), rights: CapabilityRights::READ },
        )
        .unwrap();

    new_task
        .mutable_state
        .get_mut()
        .cspace
        .mint_with_id(
            PARENT_CHANNEL,
            Capability {
                resource: CapabilityResource::Channel(parent_endpoint.clone()),
                rights: CapabilityRights::READ,
            },
        )
        .unwrap();

    for region in object.inprocess_mappings {
        task_state.memory_manager.dealloc_region(region);
    }

    let cptr = task_state.cspace.mint(Capability {
        resource: CapabilityResource::Channel(endpoint.clone()),
        rights: CapabilityRights::GRANT | CapabilityRights::WRITE | CapabilityRights::READ,
    });

    frame.a1 = cptr.value();

    SCHEDULER.enqueue(new_task);

    Ok(())
}
