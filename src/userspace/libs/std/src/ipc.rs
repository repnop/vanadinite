// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    error::SyscallError,
    syscalls::channel::{self, ReadResult},
};

pub use librust::capabilities::{
    Capability, CapabilityDescription, CapabilityPtr, CapabilityRights, CapabilityWithDescription,
};
pub use librust::syscalls::channel::{ChannelMessage, ChannelReadFlags};

#[derive(Debug)]
pub struct IpcChannel {
    cptr: CapabilityPtr,
}

impl IpcChannel {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self { cptr }
    }

    pub fn read(
        &self,
        cap_buffer: &mut [CapabilityWithDescription],
        flags: ChannelReadFlags,
    ) -> Result<ReadResult, SyscallError> {
        channel::recv(self.cptr, cap_buffer, flags)
    }

    pub fn read_with_all_caps(
        &self,
        flags: ChannelReadFlags,
    ) -> Result<(ChannelMessage, Vec<CapabilityWithDescription>), SyscallError> {
        let mut caps = Vec::new();
        let ReadResult { message, capabilities_remaining, .. } = self.read(&mut caps[..], flags)?;

        if capabilities_remaining > 0 {
            caps.resize(capabilities_remaining, CapabilityWithDescription::default());
            self.read(&mut caps[..], flags)?;
        }

        Ok((message, caps))
    }

    pub fn send(&self, msg: ChannelMessage, caps: &[Capability]) -> Result<(), SyscallError> {
        channel::send(self.cptr, msg, caps)
    }
}
