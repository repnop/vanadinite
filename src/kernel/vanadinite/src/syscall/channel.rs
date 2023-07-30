// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource},
    interrupts::PLIC,
    mem::{
        paging::{flags::Flags, VirtualAddress},
        user::{self, RawUserSlice},
    },
    scheduler::{waitqueue::WaitQueue, TASKS},
    sync::SpinMutex,
    task::Task,
    trap::GeneralRegisters,
    utils::SameHartDeadlockDetection,
    HART_ID,
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::{
        channel::{ChannelReadFlags, EndpointAlreadyMinted, EndpointIdentifier, KernelMessage},
        mem::MemoryPermissions,
    },
};

#[derive(Debug, Clone)]
pub struct ChannelEndpoint {
    queue: Arc<SpinMutex<VecDeque<(EndpointIdentifier, ChannelMessage)>, SameHartDeadlockDetection>>,
    waitqueue: WaitQueue,
    id: EndpointIdentifier,
}

impl ChannelEndpoint {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(SpinMutex::new(VecDeque::new())),
            waitqueue: WaitQueue::new(),
            id: EndpointIdentifier::UNIDENTIFIED,
        }
    }

    /// Receive a message on the endpoint, blocking until one is received
    pub fn recv(&self) -> (EndpointIdentifier, ChannelMessage) {
        let mut queue = self.queue.lock();
        loop {
            match queue.pop_front() {
                Some(msg) => break msg,
                None => self.waitqueue.wait(&mut queue),
            }
        }
    }

    pub fn try_recv(&self) -> Option<(EndpointIdentifier, ChannelMessage)> {
        self.queue.lock().pop_front()
    }

    /// Send a message on the endpoint, waking anyone waiting for a message
    pub fn send(&self, message: ChannelMessage) {
        let mut queue = self.queue.lock();
        // FIXME: this should definitely block after a certain size
        queue.push_back((self.id, message));
        self.waitqueue.wake_one();
    }

    pub fn mint(&self, new_identifier: EndpointIdentifier) -> Result<Self, EndpointAlreadyMinted> {
        match self.id {
            EndpointIdentifier::UNIDENTIFIED => {
                let mut new = self.clone();
                new.id = new_identifier;
                Ok(new)
            }
            _ => Err(EndpointAlreadyMinted),
        }
    }
}

#[derive(Clone)]
pub struct ReplyEndpoint {
    response: Arc,
}

#[derive(Debug)]
pub struct ChannelMessage {
    pub data: [usize; 7],
    pub caps: Vec<Capability>,
    pub reply_endpoint: Option<ReplyEndpoint>,
}

pub fn send(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let task_state = task.mutable_state.lock();

    let cptr = CapabilityPtr::new(frame.a1);
    let caps =
        RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(frame.a2), frame.a3);
    let data = [frame.t0, frame.t1, frame.t2, frame.t3, frame.t4, frame.t5, frame.t6];

    let channel = match task_state.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel.clone()
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match caps.len() {
        0 => Vec::new(),
        _ => {
            let cap_slice = match unsafe { caps.validate(&task_state.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return Err(SyscallError::InvalidArgument(3)),
            };

            let cap_slice = cap_slice.guarded();

            // NOTE: A capacity of 2 is used to prevent users from passing us a
            // (potentially very) large slice of invalid cptrs and causing us to
            // pre-allocate a large amount of memory that will only potentially
            // cause heap allocator pressure. Messages are unlikely to contain
            // more than 1 or 2 caps, so default to 2 as a reasonable
            // preallocation amount.
            let mut cloned_caps = Vec::with_capacity(2);
            for librust::capabilities::Capability { cptr, rights } in cap_slice.iter().copied() {
                match task_state.cspace.resolve(cptr) {
                    Some(cap) if cap.rights.is_superset(rights) && cap.rights & CapabilityRights::GRANT => {
                        // Can't allow sending invalid memory permissions
                        if let CapabilityResource::SharedMemory(..) = &cap.resource {
                            if cap.rights & CapabilityRights::WRITE && !(cap.rights & CapabilityRights::READ) {
                                return Err(SyscallError::InvalidArgument(2));
                            }
                        }

                        cloned_caps.push(cap.clone())
                    }
                    _ => return Err(SyscallError::InvalidArgument(2)),
                }
            }

            cloned_caps
        }
    };

    log::debug!("[{}:{}] Sending channel message", task.name, task.tid);
    drop(task_state);
    channel.send(ChannelMessage { data, caps });

    Ok(())
}

pub fn recv(task: &Task, regs: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let task_state = task.mutable_state.lock();

    let cptr = CapabilityPtr::new(regs.a1);
    let cap_buffer = RawUserSlice::<user::ReadWrite, librust::capabilities::CapabilityWithDescription>::new(
        VirtualAddress::new(regs.a2),
        regs.a3,
    );
    let flags = ChannelReadFlags::new(regs.a4);

    let channel = match task_state.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel.clone()
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    drop(task_state);
    let (id, ChannelMessage { data, mut caps }) = if flags & ChannelReadFlags::NONBLOCKING {
        match channel.try_recv() {
            Some(msg) => msg,
            None => return Err(SyscallError::WouldBlock),
        }
    } else {
        channel.recv()
    };

    let mut task_state = task.mutable_state.lock();

    let (caps_written, caps_remaining) = match cap_buffer.len() {
        0 => (0, caps.len()),
        len => {
            let cap_slice = match unsafe { cap_buffer.validate(&task_state.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return Err(SyscallError::InvalidArgument(3)),
            };

            let n_caps_to_write = len.min(caps.len());
            let mut cap_slice = cap_slice.guarded();
            for (target, cap) in cap_slice.iter_mut().zip(caps.drain(..n_caps_to_write)) {
                let rights = cap.rights;
                let (cptr, description) = match cap.resource {
                    CapabilityResource::Channel(channel) => (
                        task_state
                            .cspace
                            .mint(Capability { resource: CapabilityResource::Channel(channel.clone()), rights }),
                        librust::capabilities::CapabilityDescription::Channel,
                    ),
                    CapabilityResource::SharedMemory(region, _, kind) => {
                        let mut permissions = MemoryPermissions::new(0);
                        let mut memflags = Flags::VALID | Flags::USER;

                        if rights & CapabilityRights::READ {
                            permissions |= MemoryPermissions::READ;
                            memflags |= Flags::READ;
                        }

                        if rights & CapabilityRights::WRITE {
                            permissions |= MemoryPermissions::WRITE;
                            memflags |= Flags::WRITE;
                        }

                        if rights & CapabilityRights::EXECUTE {
                            permissions |= MemoryPermissions::EXECUTE;
                            memflags |= Flags::EXECUTE;
                        }

                        let addr = task_state.memory_manager.apply_shared_region(None, memflags, region.clone(), kind);

                        let cptr = task_state.cspace.mint(Capability {
                            resource: CapabilityResource::SharedMemory(region, addr.clone(), kind),
                            rights,
                        });

                        (
                            cptr,
                            librust::capabilities::CapabilityDescription::Memory {
                                ptr: addr.start.as_mut_ptr(),
                                len: addr.end.as_usize() - addr.start.as_usize(),
                                permissions,
                            },
                        )
                    }
                    CapabilityResource::Mmio(phys, _, interrupts) => {
                        // FIXME: check if this device has already been mapped
                        let virt = unsafe {
                            task_state.memory_manager.map_mmio_device(
                                phys.start,
                                None,
                                phys.end.as_usize() - phys.start.as_usize(),
                            )
                        };

                        let plic = PLIC.lock();
                        let plic = plic.as_ref().unwrap();
                        let tid = task.tid;
                        let n_interrupts = interrupts.len();
                        for interrupt in interrupts.iter().copied() {
                            // FIXME: This is copy/pasted from the `ClaimDevice` syscall, maybe
                            // refactor them both out to a function or something?
                            log::debug!("Reregistering interrupt {} to task {}", interrupt, task.name,);
                            plic.enable_interrupt(crate::platform::current_plic_context(), interrupt);
                            plic.set_context_threshold(crate::platform::current_plic_context(), 0);
                            plic.set_interrupt_priority(interrupt, 7);
                            crate::interrupts::isr::register_isr(interrupt, move |plic, _, id| {
                                plic.disable_interrupt(crate::platform::current_plic_context(), id);
                                let task = TASKS.get(tid).unwrap();
                                let mut task_state = task.mutable_state.lock();

                                log::debug!(
                                    "Interrupt {} triggered (hart: {}), notifying task {}",
                                    id,
                                    HART_ID.get(),
                                    task.name
                                );

                                task_state.claimed_interrupts.insert(id, HART_ID.get());

                                // FIXME: not sure if this is entirely correct..
                                let sender = task_state.kernel_channel.clone();

                                drop(task_state);

                                sender.send(ChannelMessage {
                                    data: Into::into(KernelMessage::InterruptOccurred(id)),
                                    caps: Vec::new(),
                                });

                                Ok(())
                            });
                        }

                        let cptr = task_state.cspace.mint(Capability {
                            resource: CapabilityResource::Mmio(phys.clone(), virt.clone(), interrupts),
                            rights,
                        });

                        (
                            cptr,
                            librust::capabilities::CapabilityDescription::MappedMmio {
                                ptr: virt.start.as_mut_ptr(),
                                len: phys.end.as_usize() - phys.start.as_usize(),
                                n_interrupts,
                            },
                        )
                    }
                };

                *target = librust::capabilities::CapabilityWithDescription {
                    capability: librust::capabilities::Capability { cptr, rights },
                    description,
                };
            }

            (n_caps_to_write, caps.len())
        }
    };

    if caps_remaining != 0 {
        channel.queue.lock().push_front((id, ChannelMessage { data: [0; 7], caps }));
    }

    regs.a1 = caps_written;
    regs.a2 = caps_remaining;
    regs.a3 = id.get();
    regs.t0 = data[0];
    regs.t1 = data[1];
    regs.t2 = data[2];
    regs.t3 = data[3];
    regs.t4 = data[4];
    regs.t5 = data[5];
    regs.t6 = data[6];

    log::debug!("[{}:{}:{:?}] Read channel message! ra={:#p}", task.name, task.tid, cptr, crate::asm::ra());
    Ok(())
}

pub fn call(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let task_state = task.mutable_state.lock();

    let cptr = CapabilityPtr::new(frame.a1);
    let caps =
        RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(frame.a2), frame.a3);
    let data = [frame.t0, frame.t1, frame.t2, frame.t3, frame.t4, frame.t5, frame.t6];

    let channel = match task_state.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel.clone()
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match caps.len() {
        0 => Vec::new(),
        _ => {
            let cap_slice = match unsafe { caps.validate(&task_state.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return Err(SyscallError::InvalidArgument(3)),
            };

            let cap_slice = cap_slice.guarded();

            // NOTE: A capacity of 2 is used to prevent users from passing us a
            // (potentially very) large slice of invalid cptrs and causing us to
            // pre-allocate a large amount of memory that will only potentially
            // cause heap allocator pressure. Messages are unlikely to contain
            // more than 1 or 2 caps, so default to 2 as a reasonable
            // preallocation amount.
            let mut cloned_caps = Vec::with_capacity(2);
            for librust::capabilities::Capability { cptr, rights } in cap_slice.iter().copied() {
                match task_state.cspace.resolve(cptr) {
                    Some(cap) if cap.rights.is_superset(rights) && cap.rights & CapabilityRights::GRANT => {
                        // Can't allow sending invalid memory permissions
                        if let CapabilityResource::SharedMemory(..) = &cap.resource {
                            if cap.rights & CapabilityRights::WRITE && !(cap.rights & CapabilityRights::READ) {
                                return Err(SyscallError::InvalidArgument(2));
                            }
                        }

                        cloned_caps.push(cap.clone())
                    }
                    _ => return Err(SyscallError::InvalidArgument(2)),
                }
            }

            cloned_caps
        }
    };
}
