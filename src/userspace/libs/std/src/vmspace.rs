// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::marker::PhantomData;

use librust::{
    capabilities::{Capability, CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::{
        channel::ChannelMessage,
        mem::MemoryPermissions,
        vmspace::{self, VmspaceObjectId, VmspaceObjectMapping, VmspaceSpawnEnv},
    },
};

pub struct Vmspace {
    name: String,
    id: VmspaceObjectId,
    names: Vec<String>,
    caps_to_send: Vec<Capability>,
}

impl Vmspace {
    #[allow(clippy::new_without_default)]
    pub fn new(name: &str) -> Self {
        let id = vmspace::create_vmspace().unwrap();

        Self { name: name.to_string(), id, names: Vec::new(), caps_to_send: Vec::new() }
    }

    pub fn create_object<'b>(
        &self,
        address: *const u8,
        size: usize,
        permissions: MemoryPermissions,
    ) -> Result<VmspaceObject<'b, '_>, SyscallError> {
        match vmspace::alloc_vmspace_object(self.id, VmspaceObjectMapping { address, size, permissions }) {
            Ok((ours, theirs)) => Ok(VmspaceObject {
                vmspace_address: theirs,
                mapped_memory: unsafe { core::slice::from_raw_parts_mut(ours, size) },
                _vmspace: PhantomData,
            }),
            Err(e) => Err(e),
        }
    }

    pub fn spawn(self, env: VmspaceSpawnEnv) -> Result<CapabilityPtr, SyscallError> {
        let cptr = vmspace::spawn_vmspace(self.id, &self.name, env)?;

        let channel = crate::ipc::IpcChannel::new(cptr);
        channel.temp_send_json(ChannelMessage::default(), &self.names, &self.caps_to_send[..])?;

        Ok(cptr)
    }

    pub fn grant(&mut self, name: &str, cptr: CapabilityPtr, rights: CapabilityRights) {
        self.names.push(name.into());
        self.caps_to_send.push(Capability { cptr, rights });
    }
}

#[derive(Debug)]
pub struct VmspaceObject<'b, 'a: 'b> {
    vmspace_address: *mut u8,
    mapped_memory: &'b mut [u8],
    _vmspace: PhantomData<&'a ()>,
}

impl<'b, 'a: 'b> VmspaceObject<'b, 'a> {
    pub fn vmspace_address(&self) -> *mut u8 {
        self.vmspace_address
    }

    pub fn as_slice(&mut self) -> &mut [u8] {
        self.mapped_memory
    }
}
