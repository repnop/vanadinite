// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    error::SyscallError,
    syscalls::channel::{self, EndpointCapability, RecvResult},
};

pub use librust::capabilities::{
    Capability, CapabilityDescription, CapabilityPtr, CapabilityRights, CapabilityWithDescription,
};
pub use librust::syscalls::channel::{ChannelReadFlags, EndpointMessage};

pub fn recv(cap_buffer: &mut [CapabilityWithDescription], flags: ChannelReadFlags) -> Result<RecvResult, SyscallError> {
    loop {
        match channel::recv(cap_buffer, flags)? {
            channel::ReadMessage::Ipc(res) => return Ok(res),
            channel::ReadMessage::Kernel(notif) => match notif {
                channel::KernelMessage::InterruptOccurred(id) => {
                    match &mut *crate::task::INTERRUPT_CALLBACK.borrow_mut() {
                        Some(callback) => callback(id),
                        None => {}
                    }
                }
            },
        }
    }
}

pub fn recv_with_all_caps(
    flags: ChannelReadFlags,
) -> Result<(EndpointMessage, Vec<CapabilityWithDescription>), SyscallError> {
    let mut caps = Vec::new();
    let RecvResult { message, capabilities_remaining, .. } = recv(&mut caps[..], flags)?;

    if capabilities_remaining > 0 {
        caps.resize(capabilities_remaining, CapabilityWithDescription::default());
        recv(&mut caps[..], flags)?;
    }

    Ok((message, caps))
}

#[derive(Debug)]
pub struct IpcChannel {
    cptr: EndpointCapability,
}

impl IpcChannel {
    pub fn new(cptr: EndpointCapability) -> Self {
        Self { cptr }
    }

    pub fn send(&self, msg: EndpointMessage, caps: &[Capability]) -> Result<(), SyscallError> {
        channel::send(self.cptr, msg, caps)
    }

    pub fn call(
        &self,
        msg: EndpointMessage,
        send_caps: &[Capability],
        recv_caps: &mut [CapabilityWithDescription],
    ) -> Result<RecvResult, SyscallError> {
        channel::call(self.cptr, msg, send_caps, recv_caps)
    }
}
