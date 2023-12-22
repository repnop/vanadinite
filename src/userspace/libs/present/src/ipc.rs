// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    executor::reactor::{BlockType, EVENT_REGISTRY, NEW_IPC_CHANNELS},
    futures::stream::{IntoStream, Stream},
};
use core::{future::Future, pin::Pin};
use librust::{
    capabilities::{Capability, CapabilityPtr, CapabilityWithDescription},
    error::SyscallError,
    syscalls::channel::{self, ChannelReadFlags, EndpointMessage, RecvResult, KERNEL_CHANNEL},
};
use std::task::{Context, Poll};

// TODO: fix all this garbage

pub struct NewChannelListener(());

impl NewChannelListener {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(())
    }
}

impl Future for NewChannelListener {
    type Output = CapabilityPtr;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut channels = NEW_IPC_CHANNELS.borrow_mut();
        match channels.is_empty() {
            false => Poll::Ready(channels.remove(0)),
            true => {
                EVENT_REGISTRY.register(BlockType::NewIpcChannel, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl Stream for NewChannelListener {
    type Item = CapabilityPtr;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        let mut channels = NEW_IPC_CHANNELS.borrow_mut();
        match channels.is_empty() {
            false => Poll::Ready(Some(channels.remove(0))),
            true => {
                EVENT_REGISTRY.register(BlockType::NewIpcChannel, context.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub struct IpcChannel(CapabilityPtr);

impl IpcChannel {
    #[track_caller]
    pub fn new(cptr: CapabilityPtr) -> Self {
        EVENT_REGISTRY.register_interest(BlockType::IpcEndpointMessage(cptr));
        Self(cptr)
    }

    pub async fn read(&self, cap_buffer: &mut [CapabilityWithDescription]) -> Result<RecvResult, SyscallError> {
        IpcRead(self, cap_buffer).await
    }

    pub async fn read_with_all_caps(&self) -> Result<(EndpointMessage, Vec<CapabilityWithDescription>), SyscallError> {
        let mut caps = Vec::new();
        let RecvResult { message, capabilities_remaining, .. } = self.read(&mut caps[..]).await?;

        if capabilities_remaining > 0 {
            caps.resize(capabilities_remaining, CapabilityWithDescription::default());
            let _ = channel::recv(self.0, &mut caps[..], ChannelReadFlags::NONBLOCKING)?;
        }

        Ok((message, caps))
    }

    pub fn send(&self, msg: EndpointMessage, caps: &[Capability]) -> Result<(), SyscallError> {
        channel::send(self.0, msg, caps)
    }
}

impl Drop for IpcChannel {
    fn drop(&mut self) {
        EVENT_REGISTRY.unregister_interest(BlockType::IpcEndpointMessage(self.0));
    }
}

impl IntoStream for IpcChannel {
    type Item = Result<(EndpointMessage, Vec<CapabilityWithDescription>), SyscallError>;
    type Stream = IpcMessageStream;

    fn into_stream(self) -> Self::Stream {
        IpcMessageStream { channel: self }
    }
}

pub struct IpcMessageStream {
    channel: IpcChannel,
}

impl Stream for IpcMessageStream {
    type Item = Result<(EndpointMessage, Vec<CapabilityWithDescription>), SyscallError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match channel::recv(this.channel.0, &mut [], ChannelReadFlags::NONBLOCKING) {
            Ok(rr) => {
                let RecvResult { message, capabilities_remaining, .. } = rr;
                let mut caps = Vec::new();

                if capabilities_remaining > 0 {
                    caps.resize(capabilities_remaining, CapabilityWithDescription::default());
                    let _ = channel::recv(this.channel.0, &mut caps[..], ChannelReadFlags::NONBLOCKING)?;
                }

                Poll::Ready(Some(Ok((message, caps))))
            }
            Err(SyscallError::WouldBlock) => {
                EVENT_REGISTRY.register(BlockType::IpcEndpointMessage(this.channel.0), context.waker().clone());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

pub struct IpcRead<'a>(&'a IpcChannel, &'a mut [CapabilityWithDescription]);

impl<'a> Future for IpcRead<'a> {
    type Output = Result<RecvResult, SyscallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        match channel::recv(this.0 .0, this.1, ChannelReadFlags::NONBLOCKING) {
            Ok(rr) => Poll::Ready(Ok(rr)),
            Err(SyscallError::WouldBlock) => {
                EVENT_REGISTRY.register(BlockType::IpcEndpointMessage(this.0 .0), cx.waker().clone());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
