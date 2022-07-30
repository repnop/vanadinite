// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    error::SyscallError,
    syscalls::{
        channel::{self, ReadResult},
        mem::{AllocationOptions, MemoryPermissions},
    },
    units::Bytes,
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
        channel::read_message(self.cptr, cap_buffer, flags)
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
        channel::send_message(self.cptr, msg, caps)
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
            channel::send_message(self.cptr, message, &[Capability { cptr, rights: CapabilityRights::READ }])
        } else {
            let mut all_caps = vec![Capability { cptr, rights: CapabilityRights::READ }];
            all_caps.extend_from_slice(other_caps);
            channel::send_message(self.cptr, message, &all_caps)
        }
    }

    pub fn temp_read_json<T: json::deser::Deserialize>(
        &self,
        flags: ChannelReadFlags,
    ) -> Result<(T, ChannelMessage, Vec<CapabilityWithDescription>), SyscallError> {
        let (msg, mut caps) = self.read_with_all_caps(flags)?;
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
