// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource},
    mem::{
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize, VirtualAddress},
        region::{MemoryRegion, PhysicalRegion},
    },
    scheduler::{Scheduler, WakeToken, CURRENT_TASK, SCHEDULER, TASKS},
    task::Task,
    utils::{self, Units},
};
use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::KError,
    message::KernelNotification,
    syscalls::channel::{ChannelId, MessageId},
};
use sync::{SpinMutex, SpinRwLock};

use super::SyscallOutcome;

pub const MAX_CHANNEL_BYTES: usize = 4096;

pub struct UserspaceChannel {
    sender: Sender,
    receiver: Receiver,
    message_id_counter: Arc<AtomicUsize>,
    mapped_regions: BTreeMap<MessageId, MappedChannelMessage>,
}

impl UserspaceChannel {
    pub fn new() -> (Self, Self) {
        let message_id_counter = Arc::new(AtomicUsize::new(0));
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
enum ChannelMessage {
    Data(MessageId, PhysicalRegion, usize),
    Capability(CapabilityPtr),
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

pub fn send_message(task: &mut Task, cptr: usize, message_id: usize, len: usize) -> SyscallOutcome {
    let channel_id = match task.cspace.resolve(CapabilityPtr::new(cptr)) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    let range = match channel.mapped_regions.remove(&MessageId::new(message_id)) {
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

    // FIXME: once buffer limits exist, will need to either block or return an
    // error and also check for broken channels
    channel.sender.try_send(ChannelMessage::Data(MessageId::new(message_id), backing, len)).unwrap();

    SyscallOutcome::Processed(librust::message::Message::default())
}

pub fn read_message(task: &mut Task, cptr: CapabilityPtr) -> SyscallOutcome {
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
    let mut lock = channel.receiver.inner.write();
    match lock.pop_front() {
        None => {
            channel.receiver.register_wake(WakeToken::new(CURRENT_TASK.get().unwrap(), move |task| {
                log::info!("Waking task for channel::read_message!");
                let res = read_message(task, cptr);
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
        Some(ChannelMessage::Data(message_id, region, len)) => {
            let region = match region {
                PhysicalRegion::Shared(region) => region,
                _ => unreachable!(),
            };

            // FIXME: make it so we can use any kind of physical region
            let region = task.memory_manager.apply_shared_region(
                None,
                flags::READ | flags::WRITE | flags::USER | flags::VALID,
                region,
                AddressRegionKind::Channel,
            );
            SyscallOutcome::processed((message_id.value(), region.start.as_usize(), len))
        }
        // Broken channel
        // FIXME: better signify this, probably needs its own error
        // FIXME: need to reinsert capability messages
        _ => SyscallOutcome::Err(KError::InvalidArgument(0)),
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

pub fn send_capability(
    task: &mut Task,
    cptr: CapabilityPtr,
    cptr_to_send: CapabilityPtr,
    rights: CapabilityRights,
) -> SyscallOutcome {
    let current_tid = CURRENT_TASK.get().unwrap();
    let cap = match task.cspace.resolve(cptr) {
        Some(cap) => cap,
        None => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };

    if !(cap.rights & CapabilityRights::GRANT) {
        return SyscallOutcome::Err(KError::InvalidArgument(0));
    }

    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };
    let (receiving_tid, receiving_channel) = task.channels.get(channel_id).unwrap();

    let cap_to_send = match task.cspace.resolve(cptr_to_send) {
        Some(cap) => cap,
        None => return SyscallOutcome::Err(KError::InvalidArgument(1)),
    };

    if !cap_to_send.rights.is_superset(rights) {
        return SyscallOutcome::Err(KError::InvalidArgument(2));
    }

    match &cap_to_send.resource {
        CapabilityResource::Channel(cid) => {
            let (other_tid, _) = task.channels.get(cid).unwrap();
            let other_task = match TASKS.get(*other_tid) {
                Some(task) => task,
                None => panic!("wut"),
            };
            let receiving_task = match TASKS.get(*receiving_tid) {
                Some(task) => task,
                None => panic!("wut"),
            };

            let mut other_task = other_task.lock();
            let mut receiving_task = receiving_task.lock();
            if other_task.state.is_dead() {
                return SyscallOutcome::Err(KError::InvalidArgument(1));
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

            other_task.message_queue.push_back((
                librust::message::Sender::kernel(),
                librust::message::Message::from(KernelNotification::ChannelOpened(other_cptr)),
            ));

            receiving_channel.sender.try_send(ChannelMessage::Capability(receiving_cptr)).unwrap();
        }
    }

    SyscallOutcome::Processed(librust::message::Message::default())
}

pub fn receive_capability(task: &mut Task, cptr: CapabilityPtr) -> SyscallOutcome {
    let channel_id = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return SyscallOutcome::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    // TODO: need to be able to return more than just the first one
    match channel.receiver.try_receive() {
        Ok(None) => SyscallOutcome::Block,
        Ok(Some(ChannelMessage::Capability(cptr))) => SyscallOutcome::processed(cptr.value()),
        // Broken channel
        // FIXME: better signify this, probably needs its own error
        // FIXME: need to reinsert data messages
        _ => SyscallOutcome::Err(KError::InvalidArgument(0)),
    }
}
