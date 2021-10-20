// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::CapabilityPtr,
    error::KError,
    message::{KernelNotification, SyscallResult},
    syscalls::{
        self,
        channel::{self, ChannelId, ChannelMessage},
        ReadMessage,
    },
    task::Tid,
};

#[derive(Debug)]
pub struct IpcChannel {
    cptr: CapabilityPtr,
}

impl IpcChannel {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self { cptr }
    }

    pub fn open(with: Tid) -> Result<Self, OpenChannelError> {
        if let SyscallResult::Err(_) = channel::request_channel(with) {
            return Err(OpenChannelError::InvalidTask);
        }

        match syscalls::receive_message() {
            Some(ReadMessage::Kernel(KernelNotification::ChannelRequestDenied)) => Err(OpenChannelError::Rejected),
            Some(ReadMessage::Kernel(KernelNotification::ChannelOpened(cptr))) => Ok(Self { cptr }),
            t => unreachable!("{:?}", t),
        }
    }

    // FIXME: use a real error
    #[allow(clippy::result_unit_err)]
    pub fn new_message(&mut self, size: usize) -> Result<NewMessage<'_>, ()> {
        let message = match channel::create_message(self.cptr, size) {
            SyscallResult::Ok(msg) => msg,
            SyscallResult::Err(_) => return Err(()),
        };

        Ok(NewMessage { channel: self, message, cursor: 0 })
    }

    // FIXME: use a real error
    #[allow(clippy::result_unit_err)]
    pub fn read(&self) -> Result<Option<Message>, KError> {
        match channel::read_message(self.cptr) {
            SyscallResult::Ok(maybe_msg) => Ok(maybe_msg.map(|m| Message(self.cptr, m))),
            SyscallResult::Err(e) => Err(e),
        }
    }

    fn send(&mut self, msg: ChannelMessage, written_len: usize) -> Result<(), SendMessageError> {
        let _ = channel::send_message(self.cptr, msg.id, written_len);
        // FIXME: check for failure
        Ok(())
    }
}

pub struct Message(CapabilityPtr, ChannelMessage);

impl Message {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.1.ptr, self.1.len) }
    }
}

impl core::fmt::Debug for Message {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Message").field("data", &self.as_bytes()).finish()
    }
}

impl core::ops::Drop for Message {
    fn drop(&mut self) {
        let _ = channel::retire_message(self.0, self.1.id);
    }
}

pub struct NewMessage<'a> {
    channel: &'a mut IpcChannel,
    message: ChannelMessage,
    cursor: usize,
}

impl NewMessage<'_> {
    pub fn send(self) -> Result<(), SendMessageError> {
        self.channel.send(self.message, self.cursor)
    }

    pub fn write(&mut self, buffer: &[u8]) {
        assert!(self.cursor + buffer.len() < self.message.len);
        let slice = unsafe {
            core::slice::from_raw_parts_mut(self.message.ptr.add(self.cursor), self.message.len - self.cursor)
        };
        slice[..buffer.len()].copy_from_slice(buffer);

        self.cursor += buffer.len();
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.message.ptr, self.message.len) }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OpenChannelError {
    InvalidTask,
    Rejected,
}

#[derive(Debug)]
pub enum SendMessageError {}
