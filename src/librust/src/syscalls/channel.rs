// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    message::{Message, MessageKind, Recipient, Sender},
    syscalls::{syscall, Syscall},
    task::Tid,
    KResult,
};
use core::convert::TryFrom;

#[repr(C)]
pub struct ChannelMessage {
    pub id: MessageId,
    pub ptr: *mut u8,
    pub len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ChannelId(usize);

impl ChannelId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct MessageId(usize);

impl MessageId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

pub fn create_channel(with: Tid) -> KResult<ChannelId> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::CreateChannel as usize,
            arguments: [with.value(), 0, 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(|m| ChannelId::new(m.arguments[0]))
}

pub fn create_message(channel: ChannelId, size: usize) -> KResult<ChannelMessage> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::CreateChannelMessage as usize,
            arguments: [channel.value(), size, 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(|m| ChannelMessage {
        id: MessageId::new(m.arguments[0]),
        ptr: m.arguments[1] as *mut u8,
        len: m.arguments[2],
    })
}

pub fn send_message(channel: ChannelId, message: MessageId, message_len: usize) -> KResult<()> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::SendChannelMessage as usize,
            arguments: [channel.value(), message.value(), message_len, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(drop)
}

pub fn read_message(channel: ChannelId) -> KResult<Option<ChannelMessage>> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::ReadChannel as usize,
            arguments: [channel.value(), 0, 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(|m| match [m.arguments[0], m.arguments[1], m.arguments[2]] {
        [0, 0, 0] => None,
        [id, ptr, len] => Some(ChannelMessage { id: MessageId::new(id), ptr: ptr as *mut u8, len }),
    })
}

pub fn retire_message(channel: ChannelId, message: MessageId) -> KResult<()> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::RetireChannelMessage as usize,
            arguments: [channel.value(), message.value(), 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(drop)
}
