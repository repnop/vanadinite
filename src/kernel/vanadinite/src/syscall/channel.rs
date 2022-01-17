// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::SyscallOutcome;
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
    HART_ID,
};
use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    vec::Vec,
};
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::KError,
    message::{KernelNotification, Message},
    syscalls::channel::{ChannelId, MessageId},
};
use sync::{SpinMutex, SpinRwLock};

pub const MAX_CHANNEL_BYTES: usize = 4096;

pub struct UserspaceChannel {
    sender: Sender,
    receiver: Receiver,
    message_id_counter: Arc<AtomicUsize>,
    mapped_regions: BTreeMap<MessageId, MappedChannelMessage>,
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
            message_id_counter: Arc::clone(&message_id_counter),
            mapped_regions: BTreeMap::new(),
        };
        let second = Self { sender: sender2, receiver: receiver1, message_id_counter, mapped_regions: BTreeMap::new() };

        (first, second)
    }

    fn next_message_id(&self) -> usize {
        self.message_id_counter.fetch_add(1, Ordering::AcqRel)
    }
}

enum MappedChannelMessage {
    Synthesized(Range<VirtualAddress>),
    Received { region: Range<VirtualAddress>, len: usize },
}

#[derive(Debug)]
struct ChannelMessage {
    data: Option<(MessageId, PhysicalRegion, usize)>,
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

// FIXME: Definitely should be a way to return tuple values that can be
// converted into `usize` so its a lot more clear what's what
pub fn create_message(task: &mut Task, cptr: CapabilityPtr, size: usize) -> SyscallOutcome {
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    let n_pages = utils::round_up_to_next(size, 4.kib()) / 4.kib();

    let message_id = channel.next_message_id();
    let size = n_pages * 4.kib();

    // FIXME: does this actually need to be shared? I don't think so
    let (region, _) = task.memory_manager.alloc_shared_region(
        None,
        RegionDescription {
            size: PageSize::Kilopage,
            len: n_pages,
            contiguous: false,
            flags: flags::READ | flags::WRITE | flags::USER | flags::VALID,
            fill: FillOption::Zeroed,
            kind: AddressRegionKind::Channel,
        },
    );

    channel.mapped_regions.insert(MessageId::new(message_id), MappedChannelMessage::Synthesized(region.clone()));

    SyscallOutcome::processed((message_id, region.start.as_usize(), size))
}

pub fn send_message(
    task: &mut Task,
    cptr: CapabilityPtr,
    message_id: MessageId,
    len: usize,
    caps: RawUserSlice<user::Read, librust::capabilities::Capability>,
) -> SyscallOutcome {
    let current_tid = task.tid;
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            *channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match caps.len() {
        0 => Vec::new(),
        _ => {
            let cap_slice = match unsafe { caps.validate(&task.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return SyscallOutcome::Err(KError::InvalidArgument(3)),
            };

            let cap_slice = cap_slice.guarded();
            let transferred_caps: Result<Vec<librust::capabilities::Capability>, KError> = cap_slice
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
                Err(e) => return SyscallOutcome::Err(e),
            }
        }
    };

    let (other_tid, channel) = task.channels.get_mut(&channel_id).unwrap();

    let range = match channel.mapped_regions.remove(&message_id) {
        Some(MappedChannelMessage::Synthesized(range)) => range,
        // For now we don't allow sending back received messages, but maybe that
        // should be allowed even if its not useful?
        _ => return SyscallOutcome::Err(KError::InvalidArgument(1)),
    };

    if range.end.as_usize() - range.start.as_usize() < len {
        return SyscallOutcome::Err(KError::InvalidArgument(2));
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

    SyscallOutcome::Processed(librust::message::Message::default())
}

pub fn read_message(
    task: &mut Task,
    cptr: CapabilityPtr,
    cap_buffer: RawUserSlice<user::ReadWrite, librust::capabilities::Capability>,
) -> SyscallOutcome {
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    // TODO: need to be able to return more than just the first one FIXME: this
    // probably needs the lock to make sure a message wasn't sent after the
    // check but before the register

    // FIXME: check for broken channel

    let mut receiver = channel.receiver.inner.write();
    match receiver.pop_front() {
        None => {
            log::debug!("Registering wake for channel::read_message");
            channel.receiver.register_wake(WakeToken::new(task.tid, move |task| {
                log::info!("Waking task {:?} (TID: {:?}) for channel::read_message!", task.name, task.tid.value());
                let res = read_message(task, cptr, cap_buffer);
                match res {
                    SyscallOutcome::Processed(message) => super::apply_message(
                        false,
                        librust::message::Sender::kernel(),
                        message,
                        &mut task.context.gp_regs,
                    ),
                    _ => todo!("is this even possible?"),
                }
            }));

            SyscallOutcome::Block
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
                        Err(_) => return SyscallOutcome::Err(KError::InvalidArgument(3)),
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

            SyscallOutcome::processed((message_id.value(), region.start.as_usize(), len, caps_written, caps_remaining))
        }
    }
}

pub fn retire_message(task: &mut Task, cptr: CapabilityPtr, message_id: MessageId) -> SyscallOutcome {
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    match channel.mapped_regions.remove(&message_id) {
        Some(MappedChannelMessage::Received { region, .. }) => {
            task.memory_manager.dealloc_region(region.start);
            SyscallOutcome::Processed(librust::message::Message::default())
        }
        _ => SyscallOutcome::Err(KError::InvalidArgument(1)),
    }
}

fn transfer_capability(
    task: &mut Task,
    cptr: CapabilityPtr,
    cptr_to_send: CapabilityPtr,
    rights: CapabilityRights,
) -> Result<CapabilityPtr, KError> {
    let current_tid = task.tid;
    let cap = match task.cspace.resolve(cptr) {
        Some(cap) => cap,
        None => return Err(KError::InvalidArgument(0)),
    };

    if !(cap.rights & CapabilityRights::GRANT) {
        return Err(KError::InvalidArgument(0));
    }

    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return Err(KError::InvalidArgument(0)),
    };

    let (receiving_tid, _) = task.channels.get(channel_id).unwrap();

    let cap_to_send = match task.cspace.resolve(cptr_to_send) {
        Some(cap) => cap,
        None => return Err(KError::InvalidArgument(1)),
    };

    if !cap_to_send.rights.is_superset(rights) {
        return Err(KError::InvalidArgument(2));
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
                return Err(KError::InvalidArgument(1));
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
                (_, _) => return Err(KError::InvalidArgument(2)),
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
