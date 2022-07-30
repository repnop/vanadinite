// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{units::Bytes, syscalls::{mem::{AllocationOptions, MemoryPermissions}, channel::{ReadResult, ChannelMessage, ChannelReadFlags, self, KERNEL_CHANNEL}}, capabilities::{Capability, CapabilityRights, CapabilityPtr, CapabilityWithDescription, CapabilityDescription}, error::SyscallError};
use crate::reactor::{BlockType, EVENT_REGISTRY, NEW_IPC_CHANNELS};
use core::{future::Future, pin::Pin};
use std::{
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
        match NEW_IPC_CHANNELS.borrow_mut().pop_front() {
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
    #[track_caller]
    pub fn new(cptr: CapabilityPtr) -> Self {
        EVENT_REGISTRY.register_interest(BlockType::IpcChannelMessage(cptr));
        Self(cptr)
    }

    pub async fn read(&self, cap_buffer: &mut [CapabilityWithDescription]) -> Result<ReadResult, SyscallError> {
        IpcRead(self, cap_buffer).await
    }

    pub async fn read_with_all_caps(&self) -> Result<(ChannelMessage, Vec<CapabilityWithDescription>), SyscallError> {
        let mut caps = Vec::new();
        let ReadResult { message, capabilities_remaining, .. } = self.read(&mut caps[..]).await?;

        if capabilities_remaining > 0 {
            caps.resize(capabilities_remaining, CapabilityWithDescription::default());
            let _ = channel::read_message(self.0, &mut caps[..], ChannelReadFlags::NONBLOCKING)?;
        }

        Ok((message, caps))
    }

    pub fn send(&self, msg: ChannelMessage, caps: &[Capability]) -> Result<(), SyscallError> {
        channel::send_message(self.0, msg, caps)
    }

    pub fn temp_send_json<T: json::deser::Serialize<Vec<u8>>>(
        &self,
        message: ChannelMessage,
        t: &T,
        other_caps: &[Capability],
    ) -> Result<(), SyscallError> {
        let serialized = json::to_bytes(t);
        let (cptr, ptr) = librust::syscalls::mem::alloc_virtual_memory(
            Bytes(serialized.len()),
            AllocationOptions::NONE,
            MemoryPermissions::READ | MemoryPermissions::WRITE,
        )?;
        unsafe { (*ptr)[..serialized.len()].copy_from_slice(&serialized) };
        if other_caps.is_empty() {
            channel::send_message(self.0, message, &[Capability { cptr, rights: CapabilityRights::READ }])
        } else {
            let mut all_caps = vec![Capability { cptr, rights: CapabilityRights::READ }];
            all_caps.extend_from_slice(other_caps);
            channel::send_message(self.0, message, &all_caps)
        }
    }

    pub async fn temp_read_json<T: json::deser::Deserialize>(
        &self,
    ) -> Result<(T, ChannelMessage, Vec<CapabilityWithDescription>), SyscallError> {
        let (msg, mut caps) = self.read_with_all_caps().await?;
        let t = match caps.remove(0) {
            CapabilityWithDescription {
                capability: _,
                description: CapabilityDescription::Memory { ptr, len, permissions: _ },
            } => json::deserialize(unsafe { core::slice::from_raw_parts(ptr, len) })
                .expect("failed to deserialize JSON in channel message"),
            _ => panic!("no or invalid mem cap"),
        };

        Ok((t, msg, caps))
    }
}

impl Drop for IpcChannel {
    fn drop(&mut self) {
        EVENT_REGISTRY.unregister_interest(BlockType::IpcChannelMessage(self.0));
    }
}

pub struct IpcRead<'a>(&'a IpcChannel, &'a mut [CapabilityWithDescription]);

impl<'a> Future for IpcRead<'a> {
    type Output = Result<ReadResult, SyscallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        match channel::read_message(this.0 .0, this.1, ChannelReadFlags::NONBLOCKING) {
            Ok(rr) => Poll::Ready(Ok(rr)),
            Err(SyscallError::WouldBlock) => {
                EVENT_REGISTRY.register(BlockType::IpcChannelMessage(this.0 .0), cx.waker().clone());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

pub async fn read_kernel_message() -> channel::KernelMessage {
    let kernel_chan = IpcChannel::new(KERNEL_CHANNEL);
    channel::KernelMessage::construct(kernel_chan.read(&mut []).await.unwrap().message.0)
}