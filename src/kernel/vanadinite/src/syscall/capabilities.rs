// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2023 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityBundle, CapabilityResource, MmioRegion, SharedMemory},
    interrupts::PLIC,
    mem::manager::AddressRegionKind,
    task::Task,
    trap::GeneralRegisters,
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::endpoint::EndpointIdentifier,
};

/// Delete a capability from a task
pub fn delete(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let cptr = CapabilityPtr::from_raw(frame.a0);

    let Some(capability) = task.cspace.remove(cptr) else {
        return Err(SyscallError::InvalidArgument(0));
    };

    match capability.resource {
        CapabilityResource::Bundle(bundle) => drop(bundle),
        CapabilityResource::Channel(channel) => drop(channel),
        CapabilityResource::SharedMemory(SharedMemory { virtual_range, kind, .. }) => {
            log::debug!("Freeing virtual memory @ {:?}", virtual_range);
            assert_eq!(kind, AddressRegionKind::UserSharedMemory);
            task.memory_manager.dealloc_region(virtual_range.start);
        }
        CapabilityResource::Mmio(MmioRegion { virtual_range, interrupts, .. }) => {
            task.memory_manager.dealloc_region(virtual_range.start);

            let plic = PLIC.lock();
            let plic = plic.as_ref().unwrap();
            for interrupt in interrupts {
                plic.disable_interrupt(crate::platform::current_plic_context(), interrupt);
                plic.set_context_threshold(crate::platform::current_plic_context(), 7);
                plic.set_interrupt_priority(interrupt, 0);

                assert!(
                    crate::interrupts::isr::unregister_isr(interrupt),
                    "attempted to unregister an ISR that was never registered?"
                );
            }
        }
        // FIXME: wake receiver up and error
        CapabilityResource::Reply(reply) => drop(reply),
    }

    Ok(())
}

pub fn mint(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let cptr = CapabilityPtr::from_raw(frame.a0);

    let Some(capability) = task.cspace.resolve(cptr) else {
        return Err(SyscallError::InvalidArgument(0));
    };

    if capability.rights & CapabilityRights::MOVE {
        return Err(SyscallError::InvalidArgument(0));
    }

    match &capability.resource {
        CapabilityResource::Channel(channel) => {
            let id = EndpointIdentifier::new(frame.a2);
            let rights = capability.rights;
            let channel = channel.mint(id).map_err(|_| SyscallError::InvalidOperation(1))?;

            frame.a1 = task.cspace.mint(Capability { resource: CapabilityResource::Channel(channel), rights }).value();

            Ok(())
        }
        CapabilityResource::Bundle(_) => Err(SyscallError::InvalidOperation(0)),
        CapabilityResource::SharedMemory(_) => Err(SyscallError::InvalidOperation(0)),
        CapabilityResource::Mmio(_) => Err(SyscallError::InvalidOperation(0)),
        CapabilityResource::Reply(_) => Err(SyscallError::InvalidArgument(0)),
    }
}

pub fn bundle(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let endpoint_badge = EndpointIdentifier::new(frame.a0);
    let endpoint_cptr = CapabilityPtr::from_raw(frame.a1);
    let shm_cptr = CapabilityPtr::from_raw(frame.a2);
    let bundle_rights = CapabilityRights::new(frame.a3);

    let Some(Capability { resource: CapabilityResource::Channel(endpoint_cap), rights: endpoint_rights }) =
        task.cspace.resolve(endpoint_cptr)
    else {
        return Err(SyscallError::InvalidArgument(1));
    };

    let Some(Capability { resource: CapabilityResource::SharedMemory(shm_cap), rights: shm_rights }) =
        task.cspace.resolve(shm_cptr)
    else {
        return Err(SyscallError::InvalidArgument(2));
    };

    // FIXME: figure out bundle rights????

    if !(*endpoint_rights & CapabilityRights::GRANT) {
        return Err(SyscallError::InvalidArgument(1));
    }

    if !(*shm_rights & CapabilityRights::GRANT) {
        return Err(SyscallError::InvalidArgument(2));
    }

    let resource = CapabilityResource::Bundle(CapabilityBundle {
        endpoint: endpoint_cap.mint(endpoint_badge).map_err(|_| SyscallError::InvalidOperation(1))?,
        shared_memory: shm_cap.clone(),
    });

    let cptr = task.cspace.mint(Capability {
        resource,
        rights: CapabilityRights::MOVE | CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
    });

    frame.a0 = cptr.value();

    Ok(())
}
