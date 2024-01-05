// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityBundle, CapabilityResource, MmioRegion, SharedMemory},
    interrupts::PLIC,
    mem::{
        paging::{flags::Flags, PhysicalAddress, VirtualAddress},
        user::{self, RawUserSlice},
    },
    scheduler::{waitqueue::WaitQueue, TASKS},
    sync::SpinMutex,
    task::{MutableState, Task},
    trap::GeneralRegisters,
    utils::SameHartDeadlockDetection,
    HART_ID,
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights, CapabilityId, CapabilityType, MemorySize},
    error::SyscallError,
    syscalls::{
        endpoint::{
            ChannelReadFlags, EndpointAlreadyMinted, EndpointIdentifier, KernelMessage, ReplyId, RECV_NO_REPLY_INFO,
            RECV_REPLY_ENDPOINT, RECV_REPLY_ID,
        },
        mem::MemoryPermissions,
    },
    Either,
};

#[derive(Debug)]
struct ChannelEndpointInner {
    queue: SpinMutex<VecDeque<(EndpointIdentifier, EndpointMessage)>, SameHartDeadlockDetection>,
    waitqueue: WaitQueue,
}

#[derive(Debug, Clone)]
pub struct ChannelEndpoint {
    inner: Arc<ChannelEndpointInner>,
    id: EndpointIdentifier,
}

impl ChannelEndpoint {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ChannelEndpointInner {
                queue: SpinMutex::new(VecDeque::new(), SameHartDeadlockDetection::new()),
                waitqueue: WaitQueue::new(),
            }),
            id: EndpointIdentifier::UNIDENTIFIED,
        }
    }

    /// Receive a message on the endpoint, blocking until one is received
    pub fn recv(&self) -> (EndpointIdentifier, EndpointMessage) {
        let mut queue = self.inner.queue.lock();
        loop {
            match queue.pop_front() {
                Some(msg) => break msg,
                None => self.inner.waitqueue.wait(&mut queue),
            }
        }
    }

    pub fn try_recv(&self) -> Option<(EndpointIdentifier, EndpointMessage)> {
        self.inner.queue.lock().pop_front()
    }

    /// Send a message on the endpoint, waking anyone waiting for a message
    pub fn send(&self, message: EndpointMessage) {
        let mut queue = self.inner.queue.lock();
        // FIXME: this should definitely block after a certain size
        queue.push_back((self.id, message));
        self.inner.waitqueue.wake_one();
    }

    pub fn mint(&self, new_identifier: EndpointIdentifier) -> Result<Self, EndpointAlreadyMinted> {
        match self.id {
            EndpointIdentifier::UNIDENTIFIED => Ok(Self { id: new_identifier, inner: Arc::clone(&self.inner) }),
            _ => Err(EndpointAlreadyMinted),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplyEndpoint(ChannelEndpoint, ReplyId);

impl ReplyEndpoint {
    pub fn new(endpoint: ChannelEndpoint, id: ReplyId) -> Self {
        Self(endpoint, id)
    }

    pub fn reply(self, mut message: EndpointMessage) {
        message.reply_endpoint = Some(Either::Right(self.1));
        self.0.send(message);
    }

    pub fn into_inner(self) -> (ChannelEndpoint, ReplyId) {
        (self.0, self.1)
    }
}

#[derive(Debug)]
pub struct EndpointMessage {
    pub data: [usize; 7],
    pub cap: Option<Capability>,
    pub reply_endpoint: Option<Either<ReplyEndpoint, ReplyId>>,
    pub shared_physical_address: Option<PhysicalAddress>,
}

pub fn send(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task_state = task.mutable_state.lock();
    // Borrowchecker is angry if its all deref'd through `task_state`, this
    // allows the borrows to be disjoint
    let MutableState { cspace, memory_manager, .. } = &mut *task_state;
    let mut reply_endpoint = frame.a4 == 1;

    let cptr = CapabilityPtr::from_raw(frame.a1);
    let cptr_to_send = CapabilityPtr::from_raw(frame.a2);
    let cptr_rights = CapabilityRights::new(frame.a3);
    let data = [frame.t0, frame.t1, frame.t2, frame.t3, frame.t4, frame.t5, frame.t6];

    let (medium, shared_physical_address) = match cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            (Either::Left(channel.clone()), None)
        }
        Some(Capability { resource: CapabilityResource::Reply(_), .. }) => {
            match cspace.remove(cptr).unwrap().resource {
                CapabilityResource::Reply(endpoint) => {
                    if reply_endpoint {
                        frame.a1 = usize::MAX;
                        reply_endpoint = false;
                    }

                    (Either::Right(endpoint), None)
                }
                _ => unreachable!(),
            }
        }
        // TODO: figure out bundle rights
        Some(Capability {
            resource: CapabilityResource::Bundle(CapabilityBundle { endpoint, shared_memory }),
            rights,
        }) => {
            for addr in shared_memory.virtual_range {
                memory_manager.modify_page_flags(addr, |_| Flags::USER | Flags::VALID);
            }

            (Either::Left(endpoint.clone()), shared_memory.physical_region.physical_addresses().next())
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let cap = match cptr_rights == CapabilityRights::NONE {
        false => Some(match cspace.resolve(cptr_to_send) {
            Some(cap) if cap.rights.is_superset(cptr_rights) && cap.rights & CapabilityRights::GRANT => {
                // Can't allow sending invalid memory permissions
                if let CapabilityResource::SharedMemory(..) = &cap.resource {
                    if cap.rights & CapabilityRights::WRITE && !(cap.rights & CapabilityRights::READ) {
                        return Err(SyscallError::InvalidArgument(2));
                    }
                }

                match cap.rights & CapabilityRights::MOVE {
                    // Remove the capability if its `MOVE`
                    true => cspace.remove(cptr).unwrap(),
                    false => cap.clone(),
                }
            }
            _ => return Err(SyscallError::InvalidArgument(2)),
        }),
        true => None,
    };

    let reply_endpoint = match reply_endpoint {
        true => {
            let id = task_state.reply_next_id.increment();
            frame.a1 = id;
            Some(Either::Left(ReplyEndpoint(task.endpoint.clone(), ReplyId::new(id))))
        }
        false => None,
    };

    log::debug!("[{}:{}] Sending channel message", task.name, task.tid);
    drop(task_state);

    let msg = EndpointMessage { data, cap, reply_endpoint, shared_physical_address };
    match medium {
        Either::Left(channel) => channel.send(msg),
        Either::Right(reply) => reply.reply(msg),
    }

    Ok(())
}

pub fn recv(task: &Task, regs: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let flags = ChannelReadFlags::new(regs.a2);
    let (id, EndpointMessage { data, cap, shared_physical_address, reply_endpoint }) =
        if flags & ChannelReadFlags::NONBLOCKING {
            match task.endpoint.try_recv() {
                Some(msg) => msg,
                None => return Err(SyscallError::WouldBlock),
            }
        } else {
            task.endpoint.recv()
        };

    let mut task_state = task.mutable_state.lock();

    if let Some(phys_addr) = shared_physical_address {
        if let Some(virt_addr) = task_state.lock_regions.get(&phys_addr) {
            for addr in virt_addr {
                task_state
                    .memory_manager
                    .modify_page_flags(addr, |_| Flags::USER | Flags::VALID | Flags::READ | Flags::WRITE);
            }
        } else {
            log::error!("[recv:{}:{}] no lock region found for physical address {:#p}", task.name, task.tid, phys_addr);
        }
    }

    let (cptr, rights) = match cap {
        Some(cap) => process_recv_cap(task, &mut *task_state, cap),
        None => (CapabilityPtr::from_raw(usize::MAX), CapabilityRights::NONE),
    };

    let (reply_value, reply_value_type) = match reply_endpoint {
        Some(Either::Left(reply)) => (
            task_state
                .cspace
                .mint(Capability {
                    resource: CapabilityResource::Reply(reply),
                    rights: CapabilityRights::GRANT | CapabilityRights::MOVE | CapabilityRights::WRITE,
                })
                .value(),
            RECV_REPLY_ENDPOINT,
        ),
        Some(Either::Right(id)) => (id.get() as usize, RECV_REPLY_ID),
        None => (0, RECV_NO_REPLY_INFO),
    };

    regs.a1 = id.get();
    regs.a2 = cptr.value();
    regs.a3 = rights.value();
    regs.a4 = reply_value;
    regs.a5 = reply_value_type;
    regs.t0 = data[0];
    regs.t1 = data[1];
    regs.t2 = data[2];
    regs.t3 = data[3];
    regs.t4 = data[4];
    regs.t5 = data[5];
    regs.t6 = data[6];

    log::debug!("[{}:{}:{:?}] Read channel message! ra={:#p}", task.name, task.tid, id, crate::asm::ra());

    Ok(())
}

pub fn call(task: &Task, regs: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let mut task_state = task.mutable_state.lock();

    let cptr = CapabilityPtr::from_raw(regs.a1);
    let read_caps =
        RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(regs.a2), regs.a3);
    let write_caps = RawUserSlice::<user::ReadWrite, librust::capabilities::CapabilityWithDescription>::new(
        VirtualAddress::new(regs.a4),
        regs.a5,
    );
    let data = [regs.t0, regs.t1, regs.t2, regs.t3, regs.t4, regs.t5, regs.t6];

    let (channel, shared_physical_address) = match task_state.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            (channel.clone(), None)
        }
        // FIXME: maybe use `rights` after figuring out bundle rights
        Some(Capability {
            resource: CapabilityResource::Bundle(CapabilityBundle { endpoint, shared_memory }),
            rights,
        }) => {
            // Shared memory permissions don't need edited here since its a synchronous call
            (endpoint.clone(), shared_memory.physical_region.physical_addresses().next())
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match read_cap_slice.len() {
        0 => Vec::new(),
        _ => {
            // NOTE: A capacity of 2 is used to prevent users from passing us a
            // (potentially very) large slice of invalid cptrs and causing us to
            // pre-allocate a large amount of memory that will only potentially
            // cause heap allocator pressure. Messages are unlikely to contain
            // more than 1 or 2 caps, so default to 2 as a reasonable
            // preallocation amount.
            let mut cloned_caps = Vec::with_capacity(2);
            for librust::capabilities::Capability { cptr, rights } in read_cap_slice.iter().copied() {
                match task_state.cspace.resolve(cptr) {
                    Some(cap) if cap.rights.is_superset(rights) && cap.rights & CapabilityRights::GRANT => {
                        // Can't allow sending invalid memory permissions
                        if let CapabilityResource::SharedMemory(..) = &cap.resource {
                            if cap.rights & CapabilityRights::WRITE && !(cap.rights & CapabilityRights::READ) {
                                return Err(SyscallError::InvalidArgument(2));
                            }
                        }

                        match cap.rights & CapabilityRights::MOVE {
                            // Remove the capability if its `MOVE`
                            true => cloned_caps.push(task_state.cspace.remove(cptr).unwrap()),
                            false => cloned_caps.push(cap.clone()),
                        }
                    }
                    _ => return Err(SyscallError::InvalidArgument(2)),
                }
            }

            cloned_caps
        }
    };

    let tmp_endpoint = ChannelEndpoint::new();
    channel.send(EndpointMessage {
        data,
        cap,
        reply_endpoint: Some(Either::Left(ReplyEndpoint::new(
            tmp_endpoint.clone(),
            ReplyId::new(task_state.reply_next_id.increment()),
        ))),
        shared_physical_address,
    });

    let (_, EndpointMessage { data, cap, .. }) = tmp_endpoint.recv();
    let (caps_written, caps_remaining) = process_recv_caps(task, &mut task_state, id, &channel, caps, write_cap_slice);

    regs.a1 = caps_written;
    regs.a2 = caps_remaining;
    regs.t0 = data[0];
    regs.t1 = data[1];
    regs.t2 = data[2];
    regs.t3 = data[3];
    regs.t4 = data[4];
    regs.t5 = data[5];
    regs.t6 = data[6];

    log::debug!("[{}:{}:{:?}] Read call reply! ra={:#p}", task.name, task.tid, cptr, crate::asm::ra());

    Ok(())
}

fn process_recv_cap(task: &Task, task_state: &mut MutableState, cap: Capability) -> (CapabilityPtr, CapabilityRights) {
    let rights = cap.rights;
    let cptr = match cap.resource {
        CapabilityResource::Reply(_) => todo!("idk if this should be a thing yet lol 🦀"),
        CapabilityResource::Bundle(CapabilityBundle {
            endpoint,
            shared_memory: SharedMemory { physical_region, kind, size .. },
            ..
        }) => {
            let addr = task_state.memory_manager.apply_shared_region(
                None,
                Flags::VALID | Flags::USER | Flags::READ | Flags::WRITE,
                physical_region.clone(),
                kind,
            );

            let cptr = CapabilityPtr::from_raw_parts(CapabilityId::from_ptr(addr.start.as_mut_ptr()), CapabilityType::Bundle(size))

            task_state
                .cspace
                .mint_with_id(
                    cptr,
                    Capability {
                        resource: CapabilityResource::Bundle(CapabilityBundle {
                            endpoint,
                            shared_memory: SharedMemory { physical_region, virtual_range: addr, kind, size },
                        }),
                        rights,
                    },
                )
                .expect("this virtual address shouldn't exist");

            (cptr, rights)
        }
        CapabilityResource::Channel(channel) => (
            task_state.cspace.mint(Capability { resource: CapabilityResource::Channel(channel), rights }),
            librust::capabilities::CapabilityDescription::Channel,
        ),
        CapabilityResource::SharedMemory(SharedMemory { physical_region, kind, size .. }) => {
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

            let addr = task_state.memory_manager.apply_shared_region(None, memflags, physical_region.clone(), kind);

            let cptr = CapabilityPtr::from_raw_parts(CapabilityId::from_ptr(addr.start.as_mut_ptr()), CapabilityType::Memory(size));

            task_state
                .cspace
                .mint_with_id(
                    cptr,
                    Capability {
                        resource: CapabilityResource::SharedMemory(SharedMemory {
                            physical_region,
                            virtual_range: addr,
                            kind,
                            size,
                        }),
                        rights,
                    },
                )
                .expect("this virtual address should not be occupied");

            cptr
        }
        CapabilityResource::Mmio(MmioRegion { physical_range, interrupts, .. }) => {
            // FIXME: check if this device has already been mapped
            let virt = unsafe {
                task_state.memory_manager.map_mmio_device(
                    physical_range.start,
                    None,
                    physical_range.end.as_usize() - physical_range.start.as_usize(),
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

                    log::debug!("Interrupt {} triggered (hart: {}), notifying task {}", id, HART_ID.get(), task.name);

                    task_state.claimed_interrupts.insert(id, HART_ID.get());
                    drop(task_state);

                    // FIXME: not sure if this is entirely correct..
                    task.endpoint.send(EndpointMessage {
                        data: Into::into(KernelMessage::InterruptOccurred(id)),
                        cap: None,
                        reply_endpoint: None,
                        shared_physical_address: None,
                    });

                    Ok(())
                });
            }

            let cptr = task_state.cspace.mint(Capability {
                resource: CapabilityResource::Mmio(MmioRegion { physical_range, virtual_range: virt, interrupts }),
                rights,
            });

            
                cptr
            
        }
    };

    (cptr, rights)
}
