// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::{Capability, CapabilityPtr, CapabilityWithDescription},
    error::SyscallError,
    syscalls::channel::{self, ChannelMessage, ChannelReadFlags, ReadResult},
};

#[derive(Debug)]
pub struct IpcChannel {
    cptr: CapabilityPtr,
}

impl IpcChannel {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self { cptr }
    }

    pub fn read(&self, cap_buffer: &mut [CapabilityWithDescription]) -> Result<ReadResult, SyscallError> {
        channel::read_message(self.cptr, cap_buffer, ChannelReadFlags::NONE)
    }

    pub fn read_with_all_caps(&self) -> Result<(ChannelMessage, Vec<CapabilityWithDescription>), SyscallError> {
        let mut caps = Vec::new();
        let ReadResult { message, capabilities_remaining, .. } = self.read(&mut caps[..])?;

        if capabilities_remaining > 0 {
            caps.resize(capabilities_remaining, CapabilityWithDescription::default());
            self.read(&mut caps[..])?;
        }

        Ok((message, caps))
    }

    pub fn send(&self, msg: ChannelMessage, caps: &[Capability]) -> Result<(), SyscallError> {
        channel::send_message(self.cptr, msg, caps)
    }
}
