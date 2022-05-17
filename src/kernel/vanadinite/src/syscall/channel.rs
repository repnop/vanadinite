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
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize, VirtualAddress},
        region::{MemoryRegion, PhysicalRegion},
        user::{self, RawUserSlice},
    },
    scheduler::{Scheduler, WakeToken, SCHEDULER, TASKS},
    task::Task,
    utils::{self, Units},
    HART_ID, trap::TrapFrame,
};
use alloc::{
    collections::{VecDeque},
    sync::Arc,
    vec::Vec,
};
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError,
};
use sync::{SpinMutex, SpinRwLock};

pub struct UserspaceChannel {
    sender: Sender,
    receiver: Receiver,
}

impl UserspaceChannel {
    pub fn new() -> (Self, Self) {
        let message_id_counter = Arc::new(AtomicUsize::new(1));
        let (sender1, receiver1) = {
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let wake = Arc::new(SpinMutex::new(None));

            let sender =
                Sender { inner: Arc::clone(&message_queue), alive: Arc::clone(&alive), wake: Arc::clone(&wake) };
            let receiver = Receiver { inner: message_queue, alive, wake };

            (sender, receiver)
        };

        let (sender2, receiver2) = {
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let wake = Arc::new(SpinMutex::new(None));

            let sender =
                Sender { inner: Arc::clone(&message_queue), alive: Arc::clone(&alive), wake: Arc::clone(&wake) };
            let receiver = Receiver { inner: message_queue, alive, wake };

            (sender, receiver)
        };

        let first = Self {
            sender: sender1,
            receiver: receiver2,
        };
        let second = Self { sender: sender2, receiver: receiver1 };

        (first, second)
    }
}

enum MappedChannelMessage {
    Synthesized(Range<VirtualAddress>),
    Received { region: Range<VirtualAddress>, len: usize },
}

#[derive(Debug)]
struct ChannelMessage {
    data: [usize; 7],
    caps: Vec<librust::capabilities::Capability>,
}

#[derive(Debug, Clone)]
struct Receiver {
    // FIXME: Replace these with something like a lockfree ring buffer
    inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    alive: Arc<AtomicBool>,
    wake: Arc<SpinMutex<Option<WakeToken>>>,
}

impl Receiver {
    fn try_receive(&self) -> Result<Option<ChannelMessage>, ()> {
        // TODO: is it worth trying to `.read()` then `.upgrade()` if not empty?
        match self.inner.write().pop_front() {
            Some(message) => Ok(Some(message)),
            None => match self.alive.load(Ordering::Acquire) {
                true => Ok(None),
                false => Err(()),
            },
        }
    }

    fn register_wake(&self, token: WakeToken) {
        self.wake.lock().replace(token);
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone)]
struct Sender {
    // FIXME: Replace these with something like a lockfree ring buffer
    inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    alive: Arc<AtomicBool>,
    wake: Arc<SpinMutex<Option<WakeToken>>>,
}

impl Sender {
    fn try_send(&self, message: ChannelMessage) -> Result<(), ChannelMessage> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(message);
        }

        // FIXME: set a buffer limit at some point
        self.inner.write().push_back(message);

        if let Some(token) = self.wake.lock().take() {
            SCHEDULER.unblock(token);
        }

        Ok(())
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

pub fn send_message(
    task: &mut Task,
    frame: &mut TrapFrame,
) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);
    let caps = RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(frame.a2), frame.a3);
    let contents = [
        frame.t0,
        frame.t1,
        frame.t2,
        frame.t3,
        frame.t4,
        frame.t5,
        frame.t6,
    ];

    let current_tid = task.tid;
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            *channel
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match caps.len() {
        0 => Vec::new(),
        _ => {
            let cap_slice = match unsafe { caps.validate(&task.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return Err(SyscallError::InvalidArgument(3)),
            };

            let cap_slice = cap_slice.guarded();
            let transferred_caps: Result<Vec<librust::capabilities::Capability>, SyscallError> = cap_slice
                .iter()
                .copied()
                .map(|cap| {
                    Ok(librust::capabilities::Capability {
                        cptr: transfer_capability(task, cptr, cap.cptr, cap.rights)?,
                        rights: cap.rights,
                    })
                })
                .collect();

            match transferred_caps {
                Ok(caps) => caps,
                Err(e) => return Err(e),
            }
        }
    };

    let (other_tid, channel) = task.channels.get_mut(&channel_id).unwrap();

    let range = match channel.mapped_regions.remove(&message_id) {
        Some(MappedChannelMessage::Synthesized(range)) => range,
        // For now we don't allow sending back received messages, but maybe that
        // should be allowed even if its not useful?
        _ => return Err(SyscallError::InvalidArgument(1)),
    };

    if range.end.as_usize() - range.start.as_usize() < len {
        return Err(SyscallError::InvalidArgument(2));
    }

    let backing = match task.memory_manager.dealloc_region(range.start) {
        MemoryRegion::Backed(phys_region) => phys_region,
        _ => unreachable!(),
    };

    let other_task = TASKS.get(*other_tid).unwrap();
    let mut other_task = other_task.lock();

    // FIXME: once buffer limits exist, will need to either block or return an
    // error and also check for broken channels
    channel.sender.try_send(ChannelMessage { data: Some((message_id, backing, len)), caps }).unwrap();

    let other_cptr = *other_task.cspace.all().find(|(_, cap)| matches!(cap, Capability { resource: CapabilityResource::Channel(cid), .. } if other_task.channels.get(cid).unwrap().0 == current_tid)).unwrap().0;
    other_task
        .message_queue
        .push(librust::message::Sender::kernel(), KernelNotification::NewChannelMessage(other_cptr).into());

    Processed(librust::message::Message::default())
}

pub fn read_message(
    task: &mut Task,
    cptr: CapabilityPtr,
    cap_buffer: RawUserSlice<user::ReadWrite, librust::capabilities::Capability>,
) -> Result<(), SyscallError> {
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    // TODO: need to be able to return more than just the first one

    // FIXME: this probably needs the lock to make sure a message wasn't sent
    // after the check but before the register

    // FIXME: check for broken channel

    let mut receiver = channel.receiver.inner.write();
    match receiver.pop_front() {
        None => {
            log::debug!("Registering wake for channel::read_message");
            channel.receiver.register_wake(WakeToken::new(task.tid, move |task| {
                log::debug!("Waking task {:?} (TID: {:?}) for channel::read_message!", task.name, task.tid.value());
                let res = read_message(task, cptr, cap_buffer);
                match res {
                    Processed(message) => super::apply_message(
                        false,
                        librust::message::Sender::kernel(),
                        message,
                        &mut task.context.gp_regs,
                    ),
                    _ => todo!("is this even possible?"),
                }
            }));

            Block
        }
        Some(ChannelMessage { data, mut caps }) => {
            let mut message_id = MessageId::new(0);
            let mut region = VirtualAddress::new(0)..VirtualAddress::new(0);
            let mut len = 0;

            if let Some((mid, mregion, mlen)) = data {
                message_id = mid;
                len = mlen;

                let mregion = match mregion {
                    PhysicalRegion::Shared(region) => region,
                    _ => unreachable!(),
                };

                // FIXME: make it so we can use any kind of physical region
                region = task.memory_manager.apply_shared_region(
                    None,
                    flags::READ | flags::WRITE | flags::USER | flags::VALID,
                    mregion,
                    AddressRegionKind::Channel,
                );
            }

            let (caps_written, caps_remaining) = match cap_buffer.len() {
                0 => (0, caps.len()),
                len => {
                    let cap_slice = match unsafe { cap_buffer.validate(&task.memory_manager) } {
                        Ok(cap_slice) => cap_slice,
                        Err(_) => return Err(SyscallError::InvalidArgument(3)),
                    };

                    let n_caps_to_write = len.min(caps.len());
                    let mut cap_slice = cap_slice.guarded();
                    for (target, cap) in cap_slice.iter_mut().zip(caps.drain(..n_caps_to_write)) {
                        *target = cap;
                    }

                    (n_caps_to_write, caps.len())
                }
            };

            if caps_remaining != 0 {
                receiver.push_front(ChannelMessage { data: None, caps });
            }

            processed((message_id.value(), region.start.as_usize(), len, caps_written, caps_remaining))
        }
    }
}

pub fn read_message_nb(
    task: &mut Task,
    cptr: CapabilityPtr,
    cap_buffer: RawUserSlice<user::ReadWrite, librust::capabilities::Capability>,
) -> Result<(), SyscallError> {
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    // TODO: need to be able to return more than just the first one FIXME: this
    // probably needs the lock to make sure a message wasn't sent after the
    // check but before the register

    // FIXME: check for broken channel

    let mut receiver = channel.receiver.inner.write();
    match receiver.pop_front() {
        None => processed((0, 0, 0, 0, 0)),
        Some(ChannelMessage { data, mut caps }) => {
            let mut message_id = MessageId::new(0);
            let mut region = VirtualAddress::new(0)..VirtualAddress::new(0);
            let mut len = 0;

            if let Some((mid, mregion, mlen)) = data {
                message_id = mid;
                len = mlen;

                let mregion = match mregion {
                    PhysicalRegion::Shared(region) => region,
                    _ => unreachable!(),
                };

                // FIXME: make it so we can use any kind of physical region
                region = task.memory_manager.apply_shared_region(
                    None,
                    flags::READ | flags::WRITE | flags::USER | flags::VALID,
                    mregion,
                    AddressRegionKind::Channel,
                );
            }

            let (caps_written, caps_remaining) = match cap_buffer.len() {
                0 => (0, caps.len()),
                len => {
                    let cap_slice = match unsafe { cap_buffer.validate(&task.memory_manager) } {
                        Ok(cap_slice) => cap_slice,
                        Err(_) => return Err(SyscallError::InvalidArgument(3)),
                    };

                    let n_caps_to_write = len.min(caps.len());
                    let mut cap_slice = cap_slice.guarded();
                    for (target, cap) in cap_slice.iter_mut().zip(caps.drain(..n_caps_to_write)) {
                        *target = cap;
                    }

                    (n_caps_to_write, caps.len())
                }
            };

            if caps_remaining != 0 {
                receiver.push_front(ChannelMessage { data: None, caps });
            }

            processed((message_id.value(), region.start.as_usize(), len, caps_written, caps_remaining))
        }
    }
}

fn transfer_capability(
    task: &mut Task,
    cptr: CapabilityPtr,
    cptr_to_send: CapabilityPtr,
    rights: CapabilityRights,
) -> Result<CapabilityPtr, SyscallError> {
    let current_tid = task.tid;
    let cap = match task.cspace.resolve(cptr) {
        Some(cap) => cap,
        None => return Err(SyscallError::InvalidArgument(0)),
    };

    if !(cap.rights & CapabilityRights::GRANT) {
        return Err(SyscallError::InvalidArgument(0));
    }

    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    let (receiving_tid, _) = task.channels.get(channel_id).unwrap();

    let cap_to_send = match task.cspace.resolve(cptr_to_send) {
        Some(cap) => cap,
        None => return Err(SyscallError::InvalidArgument(1)),
    };

    if !cap_to_send.rights.is_superset(rights) {
        return Err(SyscallError::InvalidArgument(2));
    }

    let receiving_task = match TASKS.get(*receiving_tid) {
        Some(task) => task,
        None => panic!("wut"),
    };
    let mut receiving_task = receiving_task.lock();

    match &cap_to_send.resource {
        CapabilityResource::Channel(cid) => {
            let (other_tid, _) = task.channels.get(cid).unwrap();
            let other_task = match TASKS.get(*other_tid) {
                Some(task) => task,
                None => panic!("wut"),
            };

            let mut other_task = other_task.lock();
            if other_task.state.is_dead() {
                return Err(SyscallError::InvalidArgument(1));
            }

            let other_rights = other_task
                .cspace
                .all()
                .find_map(|(_, cap)| match cap {
                    Capability { resource: CapabilityResource::Channel(id), rights } => {
                        match other_task.channels.get(id).unwrap().0 == current_tid {
                            true => Some(*rights),
                            false => None,
                        }
                    }
                    _ => None,
                })
                .unwrap();

            let receiving_task_channel_id =
                ChannelId::new(receiving_task.channels.last_key_value().map(|(id, _)| id.value() + 1).unwrap_or(0));
            let other_task_channel_id =
                ChannelId::new(other_task.channels.last_key_value().map(|(id, _)| id.value() + 1).unwrap_or(0));

            let (channel1, channel2) = UserspaceChannel::new();
            receiving_task.channels.insert(receiving_task_channel_id, (*other_tid, channel1));
            other_task.channels.insert(other_task_channel_id, (*receiving_tid, channel2));

            let receiving_cptr = receiving_task
                .cspace
                .mint(Capability { resource: CapabilityResource::Channel(receiving_task_channel_id), rights });

            let other_cptr = other_task.cspace.mint(Capability {
                resource: CapabilityResource::Channel(other_task_channel_id),
                rights: other_rights,
            });

            other_task.message_queue.push(
                librust::message::Sender::kernel(),
                librust::message::Message::from(KernelNotification::ChannelOpened(other_cptr)),
            );

            Ok(receiving_cptr)
        }
        CapabilityResource::Memory(phys_region, _, kind) => {
            let mut flags = flags::USER | flags::VALID;
            flags |= match (rights & CapabilityRights::READ, rights & CapabilityRights::WRITE) {
                (true, true) => flags::READ | flags::WRITE,
                (true, false) => flags::READ,
                // Write-only pages aren't supported & doesn't really make sense
                // to send memory the process can't use at all
                (_, _) => return Err(SyscallError::InvalidArgument(2)),
            };

            let range = receiving_task.memory_manager.apply_shared_region(None, flags, phys_region.clone(), *kind);
            let mem_cap = receiving_task
                .cspace
                .mint(Capability { rights, resource: CapabilityResource::Memory(phys_region.clone(), range, *kind) });

            Ok(mem_cap)
        }
        CapabilityResource::Mmio(..) => {
            let cap = task.cspace.remove(cptr_to_send).unwrap();
            let (vregion, interrupts) = match cap.resource {
                CapabilityResource::Mmio(vregion, interrupts) => (vregion, interrupts),
                _ => unreachable!(),
            };

            let region = match task.memory_manager.dealloc_region(vregion.start) {
                MemoryRegion::Backed(region) => region,
                _ => unreachable!(),
            };

            // FIXME: need to change `map_mmio_device` to probably accept a
            // new-typed size or something to make it more obvious its length
            // and not page count
            let size = region.page_count() * 4.kib();
            let start = region.physical_addresses().next().unwrap();
            // We know at this point that its been removed from the previous
            // process and MMIO caps are unique in a system
            let vrange = unsafe { receiving_task.memory_manager.map_mmio_device(start, None, size) };

            // We want to avoid a possible race here, we want the task to know
            // about the MMIO device capability _before_ any interrupts occur
            //
            // May also need to consider disabling these interrupts before
            // transferring the cap so interrupts aren't lost, but I think for
            // now that shouldn't be an issue since ideally the devices aren't
            // initialized until they're received by the final recipient
            let receiving_cptr = receiving_task
                .cspace
                .mint(Capability { resource: CapabilityResource::Mmio(vrange, interrupts.clone()), rights });

            let plic = PLIC.lock();
            let plic = plic.as_ref().unwrap();
            let receiving_tid = *receiving_tid;
            for interrupt in interrupts {
                // FIXME: This is copy/pasted from the `ClaimDevice` syscall, maybe
                // refactor them both out to a function or something?
                log::debug!(
                    "Reregistering interrupt {} from task {} to task {}",
                    interrupt,
                    task.name,
                    receiving_task.name
                );
                plic.enable_interrupt(crate::platform::current_plic_context(), interrupt);
                plic.set_context_threshold(crate::platform::current_plic_context(), 0);
                plic.set_interrupt_priority(interrupt, 7);
                crate::interrupts::isr::register_isr(interrupt, move |plic, _, id| {
                    plic.disable_interrupt(crate::platform::current_plic_context(), id);
                    let task = TASKS.get(receiving_tid).unwrap();
                    let mut task = task.lock();

                    log::debug!("Interrupt {} triggered (hart: {}), notifying task {}", id, HART_ID.get(), task.name);

                    task.claimed_interrupts.insert(id, HART_ID.get());
                    task.message_queue.push(
                        librust::message::Sender::kernel(),
                        Message::from(KernelNotification::InterruptOccurred(id)),
                    );

                    Ok(())
                });
            }

            Ok(receiving_cptr)
        }
    }
}
