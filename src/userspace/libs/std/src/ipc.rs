// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::{Capability, CapabilityPtr},
    error::KError,
    message::SyscallResult,
    syscalls::channel::{self, ChannelMessage},
};

#[derive(Debug)]
pub struct IpcChannel {
    cptr: CapabilityPtr,
}

impl IpcChannel {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self { cptr }
    }

    // FIXME: use a real error
    #[allow(clippy::result_unit_err)]
    pub fn new_message(&mut self, size: usize) -> Result<NewMessage<'_>, KError> {
        let message = match channel::create_message(self.cptr, size) {
            SyscallResult::Ok(msg) => msg,
            SyscallResult::Err(e) => return Err(e),
        };

        Ok(NewMessage { channel: self, message, cursor: 0 })
    }

    pub fn send_bytes<T: AsRef<[u8]>>(&mut self, msg: T, caps: &[Capability]) -> Result<(), KError> {
        let msg = msg.as_ref();
        let mut chan_msg = self.new_message(msg.len())?;
        chan_msg.write(msg);
        chan_msg.send(caps)
    }

    // FIXME: use a real error
    #[allow(clippy::result_unit_err)]
    pub fn read(&self, cap_buffer: &mut [Capability]) -> Result<ReadChannelMessage, KError> {
        match channel::read_message(self.cptr, cap_buffer) {
            SyscallResult::Ok((m, caps_read, caps_left)) => {
                Ok(ReadChannelMessage { message: Message(self.cptr, m), caps_read, caps_left })
            }
            SyscallResult::Err(e) => Err(e),
        }
    }

    pub fn read_with_all_caps(&self) -> Result<(Message, Vec<Capability>), KError> {
        let mut caps = Vec::new();
        let ReadChannelMessage { message, caps_left, .. } = self.read(&mut caps[..])?;

        if caps_left > 0 {
            caps.resize(caps_left, Capability::default());
            self.read(&mut caps[..])?;
        }

        Ok((message, caps))
    }

    fn send(&mut self, msg: ChannelMessage, written_len: usize, caps: &[Capability]) -> Result<(), KError> {
        if let SyscallResult::Err(e) = channel::send_message(self.cptr, msg.id, written_len, caps) {
            return Err(e);
        }

        // FIXME: check for failure
        Ok(())
    }
}

pub struct ReadChannelMessage {
    pub message: Message,
    pub caps_read: usize,
    pub caps_left: usize,
}

pub struct Message(CapabilityPtr, ChannelMessage);

impl Message {
    pub fn as_bytes(&self) -> &[u8] {
        if !self.1.ptr.is_null() {
            unsafe { core::slice::from_raw_parts(self.1.ptr, self.1.len) }
        } else {
            &[]
        }
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
    pub fn send(self, caps: &[Capability]) -> Result<(), KError> {
        self.channel.send(self.message, self.cursor, caps)
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
