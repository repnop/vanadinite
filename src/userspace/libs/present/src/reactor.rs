// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    collections::BTreeMap,
    librust::{
        capabilities::CapabilityPtr,
        message::KernelNotification,
        syscalls::{receive_message, ReadMessage},
    },
    task::Waker,
};

use sync::{Immediate, Lazy, SpinMutex};

pub(crate) static NEW_IPC_CHANNELS: SpinMutex<Lazy<VecDeque<CapabilityPtr>>> = SpinMutex::new(Lazy::new(VecDeque::new));
pub(crate) static EVENT_REGISTRY: EventRegistry = EventRegistry::new();

pub struct EventRegistry {
    waiting_for_event: SpinMutex<BTreeMap<BlockType, Waker>, Immediate>,
    interest: SpinMutex<BTreeMap<BlockType, usize>, Immediate>,
}

impl EventRegistry {
    pub(crate) const fn new() -> Self {
        Self { waiting_for_event: SpinMutex::new(BTreeMap::new()), interest: SpinMutex::new(BTreeMap::new()) }
    }

    pub(crate) fn register_interest(&self, block_type: BlockType) {
        assert!(self.interest.lock().insert(block_type, 0).is_none());
    }

    pub(crate) fn unregister_interest(&self, block_type: BlockType) {
        assert!(self.interest.lock().remove(&block_type).is_none());
    }

    pub(crate) fn is_interest(&self, block_type: BlockType) -> bool {
        self.interest.lock().get(&block_type).is_some()
    }

    pub(crate) fn add_interested_event(&self, block_type: BlockType) {
        if let Some(n) = self.interest.lock().get_mut(&block_type) {
            *n += 1;
        }
    }

    pub(crate) fn consume_interest_event(&self, block_type: BlockType) -> bool {
        match self.interest.lock().get_mut(&block_type) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }

    #[track_caller]
    pub(crate) fn register(&self, block_type: BlockType, waker: Waker) {
        // TODO: figure out if its okay to ignore adding more than one thing
        // here...
        self.waiting_for_event.lock().insert(block_type, waker);
    }

    pub(crate) fn unregister(&self, block_type: BlockType) -> Option<Waker> {
        self.waiting_for_event.lock().remove(&block_type)
    }

    #[track_caller]
    pub(crate) fn wake(&self, block_type: BlockType) {
        self.unregister(block_type).expect("blocked task doesn't exist").wake();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BlockType {
    NewIpcChannel,
    IpcChannelMessage(CapabilityPtr),
    Interrupt(usize),
    AsyncChannel(u64),
}

pub struct Reactor;

impl Reactor {
    pub fn wait() {
        match receive_message() {
            ReadMessage::Kernel(KernelNotification::InterruptOccurred(id)) => {
                EVENT_REGISTRY.add_interested_event(BlockType::Interrupt(id));
                if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::Interrupt(id)) {
                    waker.wake();
                }
            }
            ReadMessage::Kernel(KernelNotification::NewChannelMessage(cptr)) => {
                EVENT_REGISTRY.add_interested_event(BlockType::IpcChannelMessage(cptr));
                if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::IpcChannelMessage(cptr)) {
                    waker.wake();
                }
            }
            ReadMessage::Kernel(KernelNotification::ChannelOpened(cptr)) => {
                let mut new_ipc_channels = NEW_IPC_CHANNELS.lock();
                new_ipc_channels.push_back(cptr);
                if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::NewIpcChannel) {
                    drop(new_ipc_channels);
                    waker.wake();
                }
            }
            _ => {}
        }
    }
}
