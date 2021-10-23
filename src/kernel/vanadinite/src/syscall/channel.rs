// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource, CapabilityRights},
    mem::{
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize, VirtualAddress},
        region::{MemoryRegion, PhysicalRegion},
    },
    scheduler::{CURRENT_TASK, TASKS},
    task::{Task, TaskState},
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
    capabilities::CapabilityPtr,
    error::KError,
    message::{KernelNotification, SyscallResult},
    syscalls::channel::{ChannelId, MessageId},
    task::Tid,
};
use sync::SpinRwLock;

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

            let sender = Sender { inner: Arc::clone(&message_queue), alive: Arc::clone(&alive) };
            let receiver = Receiver { inner: message_queue, alive };

            (sender, receiver)
        };

        let (sender2, receiver2) = {
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));

            let sender = Sender { inner: Arc::clone(&message_queue), alive: Arc::clone(&alive) };
            let receiver = Receiver { inner: message_queue, alive };

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
    Capability(CapabilityResource, CapabilityRights),
}

#[derive(Debug, Clone)]
struct Receiver {
    // FIXME: Replace these with something like a lockfree ring buffer
    inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    alive: Arc<AtomicBool>,
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
}

impl Sender {
    fn try_send(&self, message: ChannelMessage) -> Result<(), ChannelMessage> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(message);
        }

        // FIXME: set a buffer limit at some point
        self.inner.write().push_back(message);
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
pub fn create_message(task: &mut Task, cptr: usize, size: usize) -> SyscallResult<(usize, usize, usize), KError> {
    let channel_id = match task.cspace.resolve(CapabilityPtr::new(cptr)) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return SyscallResult::Err(KError::InvalidArgument(0)),
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

    SyscallResult::Ok((message_id, region.start.as_usize(), size))
}

pub fn send_message(task: &mut Task, cptr: usize, message_id: usize, len: usize) -> SyscallResult<(), KError> {
    let channel_id = match task.cspace.resolve(CapabilityPtr::new(cptr)) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return SyscallResult::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    let range = match channel.mapped_regions.remove(&MessageId::new(message_id)) {
        Some(MappedChannelMessage::Synthesized(range)) => range,
        // For now we don't allow sending back received messages, but maybe that
        // should be allowed even if its not useful?
        _ => return SyscallResult::Err(KError::InvalidArgument(1)),
    };

    if range.end.as_usize() - range.start.as_usize() < len {
        return SyscallResult::Err(KError::InvalidArgument(2));
    }

    let backing = match task.memory_manager.dealloc_region(range.start) {
        MemoryRegion::Backed(phys_region) => phys_region,
        _ => unreachable!(),
    };

    // FIXME: once buffer limits exist, will need to either block or return an
    // error and also check for broken channels
    channel.sender.try_send(ChannelMessage::Data(MessageId::new(message_id), backing, len)).unwrap();

    SyscallResult::Ok(())
}

pub fn read_message(task: &mut Task, cptr: usize) -> SyscallResult<(usize, usize, usize), KError> {
    let channel_id = match task.cspace.resolve(CapabilityPtr::new(cptr)) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return SyscallResult::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    // TODO: need to be able to return more than just the first one
    match channel.receiver.try_receive() {
        Ok(None) => SyscallResult::Ok((0, 0, 0)),
        Ok(Some(ChannelMessage::Data(message_id, region, len))) => {
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
            SyscallResult::Ok((message_id.value(), region.start.as_usize(), len))
        }
        // Broken channel
        // FIXME: better signify this, probably needs its own error
        _ => SyscallResult::Err(KError::InvalidArgument(0)),
    }
}

pub fn retire_message(task: &mut Task, cptr: usize, message_id: usize) -> SyscallResult<(), KError> {
    let channel_id = match task.cspace.resolve(CapabilityPtr::new(cptr)) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return SyscallResult::Err(KError::InvalidArgument(0)),
    };
    let (_, channel) = task.channels.get_mut(channel_id).unwrap();

    match channel.mapped_regions.remove(&MessageId::new(message_id)) {
        Some(MappedChannelMessage::Received { region, .. }) => {
            task.memory_manager.dealloc_region(region.start);
            SyscallResult::Ok(())
        }
        _ => SyscallResult::Err(KError::InvalidArgument(1)),
    }
}
