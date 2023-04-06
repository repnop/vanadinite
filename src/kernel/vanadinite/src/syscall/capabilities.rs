// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2023 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::CapabilityResource, interrupts::PLIC, mem::manager::AddressRegionKind, task::Task,
    trap::GeneralRegisters,
};
use librust::{capabilities::CapabilityPtr, error::SyscallError};

/// Delete a capability from a task
pub fn delete(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task = task.mutable_state.lock();
    let cptr = CapabilityPtr::new(frame.a0);

    let Some(capability) = task.cspace.remove(cptr) else {
        return Err(SyscallError::InvalidArgument(0));
    };

    match capability.resource {
        CapabilityResource::Channel(channel) => drop(channel),
        CapabilityResource::SharedMemory(_, range, kind) => {
            log::debug!("Freeing virtual memory @ {:?}", range);
            assert_eq!(kind, AddressRegionKind::UserSharedMemory);
            task.memory_manager.dealloc_region(range.start);
        }
        CapabilityResource::Mmio(_, range, interrupts) => {
            task.memory_manager.dealloc_region(range.start);

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
    }

    Ok(())
}
