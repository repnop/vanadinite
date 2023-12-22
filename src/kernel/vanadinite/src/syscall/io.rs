// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource},
    interrupts::PLIC,
    io::CLAIMED_DEVICES,
    mem::{
        paging::{PageSize, PhysicalAddress, VirtualAddress},
        user::RawUserSlice,
        PageRange,
    },
    platform::FDT,
    scheduler::TASKS,
    syscall::channel::EndpointMessage,
    task::Task,
    trap::GeneralRegisters,
    HART_ID,
};
use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use librust::{capabilities::CapabilityRights, error::SyscallError, syscalls::channel::KernelMessage};

pub fn claim_device(task: &Task, regs: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task_state = task.mutable_state.lock();
    let start = VirtualAddress::new(regs.a1);
    let len = regs.a2;
    let user_slice = RawUserSlice::readable(start, len);
    let user_slice = match unsafe { user_slice.validate(&task_state.memory_manager) } {
        Ok(slice) => slice,
        Err((_, e)) => {
            log::error!("Bad memory from process: {:?}", e);
            return Err(SyscallError::InvalidArgument(0));
        }
    };

    let slice = user_slice.guarded();
    let node_path = match core::str::from_utf8(&slice) {
        Ok(s) => s,
        Err(_) => {
            log::error!("Invalid UTF-8 in FDT node name from process");
            return Err(SyscallError::InvalidArgument(0));
        }
    };

    // FIXME: make better errors
    let claimed = CLAIMED_DEVICES.read();
    if claimed.get(node_path).is_some() {
        return Err(SyscallError::InvalidArgument(0));
    }

    let fdt = unsafe { fdt::Fdt::from_ptr(FDT.load(Ordering::Acquire)) }.unwrap();

    // FIXME: probably should add some sanity checks for what we're
    // mapping
    //
    // FIXME: `fdt` needs updated so that we can get the full node path,
    // so work around that temporarily here
    let mut all_nodes = fdt.all_nodes();
    match all_nodes.find(|n| n.name == node_path) {
        Some(node) => {
            // FIXME: what about multiple regions?
            match node.reg().into_iter().flatten().next() {
                Some(fdt::standard_nodes::MemoryRegion { size: Some(len), starting_address }) => {
                    claimed.upgrade().insert(node_path.into(), task.tid);
                    let map_to = unsafe {
                        task_state.memory_manager.map_mmio_device(
                            PhysicalAddress::from_ptr(starting_address),
                            None,
                            len,
                        )
                    };

                    // FIXME: this probably needs marked as `Clone` in
                    // `fdt` or something
                    let interrupts = node.interrupts().into_iter().flatten();
                    let physical_range = {
                        let start = PhysicalAddress::from_ptr(starting_address);
                        PageRange::new(start, start.offset(len), PageSize::Kilopage)
                    };
                    let cptr = task_state.cspace.mint(Capability {
                        resource: CapabilityResource::Mmio(crate::capabilities::MmioRegion {
                            physical_range,
                            virtual_range: map_to,
                            interrupts: interrupts.collect(),
                        }),
                        rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
                    });

                    let current_tid = task.tid;
                    let interrupts = node.interrupts().into_iter().flatten();

                    let plic = PLIC.lock();
                    let plic = plic.as_ref().unwrap();
                    for interrupt in interrupts {
                        log::debug!("Giving interrupt {} to task {}", interrupt, task.name);
                        plic.enable_interrupt(crate::platform::current_plic_context(), interrupt);
                        plic.set_context_threshold(crate::platform::current_plic_context(), 0);
                        plic.set_interrupt_priority(interrupt, 7);
                        crate::interrupts::isr::register_isr(interrupt, move |plic, _, id| {
                            plic.disable_interrupt(crate::platform::current_plic_context(), id);
                            let task = TASKS.get(current_tid).unwrap();
                            let mut task_state = task.mutable_state.lock();

                            log::debug!(
                                "Interrupt {} triggered (hart: {}), notifying task {}",
                                id,
                                HART_ID.get(),
                                task.name
                            );

                            task_state.claimed_interrupts.insert(id, HART_ID.get());
                            drop(task_state);

                            // FIXME: not sure if this is entirely correct..
                            task.endpoint.send(EndpointMessage {
                                data: Into::into(KernelMessage::InterruptOccurred(id)),
                                caps: Vec::new(),
                                reply_endpoint: None,
                                shared_physical_address: None,
                            });

                            Ok(())
                        });
                    }

                    regs.a1 = cptr.value();
                    Ok(())
                }
                _ => Err(SyscallError::InvalidArgument(0)),
            }
        }
        None => Err(SyscallError::InvalidArgument(0)),
    }
}

pub fn complete_interrupt(task: &Task, regs: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let interrupt_id = regs.a1;
    match task.mutable_state.lock().claimed_interrupts.remove(&interrupt_id) {
        None => Err(SyscallError::InvalidArgument(0)),
        Some(hart) => {
            log::debug!("Task {} completing interrupt {}", task.name, interrupt_id);
            if let Some(plic) = &*PLIC.lock() {
                plic.complete(crate::platform::plic_context_for(hart), interrupt_id);
                plic.enable_interrupt(crate::platform::plic_context_for(hart), interrupt_id);
            }

            Ok(())
        }
    }
}
