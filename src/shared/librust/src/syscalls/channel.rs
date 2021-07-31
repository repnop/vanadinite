// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    error::KError,
    message::{Recipient, SyscallRequest, SyscallResult},
    syscalls::{syscall, Syscall},
    task::Tid,
};

#[derive(Debug, Clone, Copy)]
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

pub fn request_channel(with: Tid) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest { syscall: Syscall::RequestChannel, arguments: [with.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    )
    .1
}

pub fn create_channel(with: Tid) -> SyscallResult<ChannelId, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest { syscall: Syscall::CreateChannel, arguments: [with.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    )
    .1
    .map(ChannelId)
}

pub fn create_message(channel: ChannelId, size: usize) -> SyscallResult<ChannelMessage, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::CreateChannelMessage,
            arguments: [channel.value(), size, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|(id, ptr, len)| ChannelMessage { id: MessageId::new(id), ptr: ptr as *mut u8, len })
}

pub fn send_message(channel: ChannelId, message: MessageId, message_len: usize) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::SendChannelMessage,
            arguments: [channel.value(), message.value(), message_len, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
}

pub fn read_message(channel: ChannelId) -> SyscallResult<Option<ChannelMessage>, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest { syscall: Syscall::ReadChannel, arguments: [channel.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    )
    .1
    .map(|res| match res {
        (0, 0, 0) => None,
        (id, ptr, len) => Some(ChannelMessage { id: MessageId::new(id), ptr: ptr as *mut u8, len }),
    })
}

pub fn retire_message(channel: ChannelId, message: MessageId) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::RetireChannelMessage,
            arguments: [channel.value(), message.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
}
