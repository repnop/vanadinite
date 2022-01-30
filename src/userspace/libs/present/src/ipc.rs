// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::reactor::{BlockType, EVENT_REGISTRY, NEW_IPC_CHANNELS};
use core::{future::Future, pin::Pin};
use std::{
    ipc::{Message, ReadChannelMessage},
    librust::{
        capabilities::{Capability, CapabilityPtr},
        error::KError,
        message::SyscallResult,
        syscalls::{self, channel::ChannelMessage},
    },
    task::{Context, Poll},
};

// TODO: fix all this garbage

pub struct NewChannelListener(());

impl NewChannelListener {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(())
    }

    pub async fn recv(&self) -> CapabilityPtr {
        NewChannelListenerRecv.await
    }
}

struct NewChannelListenerRecv;

impl Future for NewChannelListenerRecv {
    type Output = CapabilityPtr;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match NEW_IPC_CHANNELS.lock().pop_front() {
            Some(cptr) => Poll::Ready(cptr),
            None => {
                EVENT_REGISTRY.register(BlockType::NewIpcChannel, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub struct IpcChannel(CapabilityPtr);

impl IpcChannel {
    pub fn new(cptr: CapabilityPtr) -> Self {
        EVENT_REGISTRY.register_interest(BlockType::IpcChannelMessage(cptr));
        Self(cptr)
    }

    // FIXME: use a real error
    #[allow(clippy::result_unit_err)]
    pub fn new_message(&mut self, size: usize) -> Result<NewMessage<'_>, KError> {
        let message = match syscalls::channel::create_message(self.0, size) {
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
    pub fn read<'a>(&'a self, cap_buffer: &'a mut [Capability]) -> IpcRead<'a> {
        IpcRead(self, cap_buffer)
    }

    pub async fn read_with_all_caps(&self) -> Result<(Message, Vec<Capability>), KError> {
        let mut caps = vec![Capability::default(); 4];
        let ReadChannelMessage { message, caps_left, caps_read } = self.read(&mut caps[..]).await?;

        if caps_left > 0 {
            caps.resize(caps_left + caps_read, Capability::default());
            if let SyscallResult::Err(e) = dbg!(syscalls::channel::read_message_non_blocking(self.0, &mut caps[caps_read..]))
            {
                return Err(e);
            }
        } else {
            caps.truncate(caps_read);
        }

        Ok((message, caps))
    }

    fn send(&mut self, msg: ChannelMessage, written_len: usize, caps: &[Capability]) -> Result<(), KError> {
        if let SyscallResult::Err(e) = syscalls::channel::send_message(self.0, msg.id, written_len, caps) {
            return Err(e);
        }

        // FIXME: check for failure
        Ok(())
    }
}

impl Drop for IpcChannel {
    fn drop(&mut self) {
        EVENT_REGISTRY.unregister_interest(BlockType::IpcChannelMessage(self.0));
    }
}

pub struct IpcRead<'a>(&'a IpcChannel, &'a mut [Capability]);

impl<'a> Future for IpcRead<'a> {
    type Output = Result<ReadChannelMessage, KError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        match EVENT_REGISTRY.consume_interest_event(BlockType::IpcChannelMessage(this.0 .0)) {
            true => {
                match syscalls::channel::read_message_non_blocking(this.0 .0, this.1) {
                    SyscallResult::Ok(Some((m, caps_read, caps_left))) => Poll::Ready(Ok(ReadChannelMessage {
                        message: unsafe { Message::new(this.0 .0, m) },
                        caps_read,
                        caps_left,
                    })),
                    SyscallResult::Err(e) => Poll::Ready(Err(e)),
                    _ => unreachable!()
                }
            }
            false => {
                EVENT_REGISTRY.register(BlockType::IpcChannelMessage(this.0 .0), cx.waker().clone());
                Poll::Pending
            }
        }
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
