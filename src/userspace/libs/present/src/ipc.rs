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
    capabilities::{Capability, CapabilityPtr},
    error::SyscallError,
    syscalls::endpoint::{self, ChannelReadFlags, EndpointCapability, EndpointMessage, IpcMessage},
};
use std::task::{Context, Poll};

// TODO: fix all this garbage

pub struct IpcChannel(EndpointCapability);

impl IpcChannel {
    #[track_caller]
    pub fn new(cptr: EndpointCapability) -> Self {
        EVENT_REGISTRY.register_interest(BlockType::IpcEndpointMessage(cptr));
        Self(cptr)
    }

    pub async fn read(&self) -> Result<IpcMessage, SyscallError> {
        IpcRead(self, cap_buffer).await
    }

    pub fn send(&self, msg: EndpointMessage, caps: Option<Capability>) -> Result<(), SyscallError> {
        endpoint::send(self.0, msg, caps)
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

        match endpoint::recv(this.channel.0, ChannelReadFlags::NONBLOCKING) {
            Ok(rr) => {
                let IpcMessage { message, capabilities_remaining, .. } = rr;
                let mut caps = Vec::new();

                if capabilities_remaining > 0 {
                    caps.resize(capabilities_remaining, CapabilityWithDescription::default());
                    let _ = endpoint::recv(this.channel.0, &mut caps[..], ChannelReadFlags::NONBLOCKING)?;
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
    type Output = Result<IpcMessage, SyscallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        match endpoint::recv(this.0 .0, this.1, ChannelReadFlags::NONBLOCKING) {
            Ok(rr) => Poll::Ready(Ok(rr)),
            Err(SyscallError::WouldBlock) => {
                EVENT_REGISTRY.register(BlockType::IpcEndpointMessage(this.0 .0), cx.waker().clone());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
