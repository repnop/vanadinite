// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::CapabilityPtr,
    syscalls::endpoint::{KernelMessage, Message},
};
use std::{collections::BTreeMap, sync::SyncRefCell, task::Waker};

pub(crate) static EVENT_REGISTRY: EventRegistry = EventRegistry::new();
pub(crate) static SEEN_IPC_CHANNELS: SyncRefCell<BTreeMap<CapabilityPtr, ()>> = SyncRefCell::new(BTreeMap::new());
pub(crate) static NEW_IPC_CHANNELS: SyncRefCell<Vec<CapabilityPtr>> = SyncRefCell::new(Vec::new());

pub struct EventRegistry {
    waiting_for_event: SyncRefCell<BTreeMap<BlockType, Waker>>,
    interest: SyncRefCell<BTreeMap<BlockType, usize>>,
}

impl EventRegistry {
    pub(crate) const fn new() -> Self {
        Self { waiting_for_event: SyncRefCell::new(BTreeMap::new()), interest: SyncRefCell::new(BTreeMap::new()) }
    }

    #[track_caller]
    pub(crate) fn register_interest(&self, block_type: BlockType) {
        assert!(self.interest.borrow_mut().insert(block_type, 0).is_none());
    }

    pub(crate) fn unregister_interest(&self, block_type: BlockType) {
        assert!(self.interest.borrow_mut().remove(&block_type).is_none());
    }

    pub(crate) fn add_interested_event(&self, block_type: BlockType) {
        if let Some(n) = self.interest.borrow_mut().get_mut(&block_type) {
            *n += 1;
        }
    }

    pub(crate) fn consume_interest_event(&self, block_type: BlockType) -> bool {
        match self.interest.borrow_mut().get_mut(&block_type) {
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
        self.waiting_for_event.borrow_mut().insert(block_type, waker);
    }

    pub(crate) fn unregister(&self, block_type: BlockType) -> Option<Waker> {
        self.waiting_for_event.borrow_mut().remove(&block_type)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BlockType {
    IpcEndpointMessage(CapabilityPtr),
    Interrupt(usize),
}

pub struct Reactor;

impl Reactor {
    pub fn wait() {
        loop {
            let Ok(msg) = librust::syscalls::endpoint::recv() else { continue };
            match msg {
                Message::Kernel(KernelMessage::InterruptOccurred(id)) => {
                    EVENT_REGISTRY.add_interested_event(BlockType::Interrupt(id));
                    if let Some(waker) = EVENT_REGISTRY.unregister(BlockType::Interrupt(id)) {
                        waker.wake();
                    }
                }
                Message::Ipc(msg)
            }
        }
    }
}
