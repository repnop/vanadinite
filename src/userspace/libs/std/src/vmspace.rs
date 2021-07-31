// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::marker::PhantomData;

use librust::{
    error::KError,
    message::SyscallResult,
    syscalls::{
        allocation::MemoryPermissions,
        vmspace::{self, VmspaceObjectId, VmspaceObjectMapping, VmspaceSpawnEnv},
    },
    task::Tid,
};

pub struct Vmspace {
    id: VmspaceObjectId,
}

impl Vmspace {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let id = vmspace::create_vmspace().unwrap();

        Self { id }
    }

    pub fn create_object<'b>(
        &self,
        address: *const u8,
        size: usize,
        permissions: MemoryPermissions,
    ) -> Result<VmspaceObject<'b, '_>, KError> {
        match vmspace::alloc_vmspace_object(self.id, VmspaceObjectMapping { address, size, permissions }) {
            SyscallResult::Ok((ours, theirs)) => Ok(VmspaceObject {
                vmspace_address: theirs,
                mapped_memory: unsafe { core::slice::from_raw_parts_mut(ours, size) },
                _vmspace: PhantomData,
            }),
            SyscallResult::Err(e) => Err(e),
        }
    }

    pub fn spawn(self, env: VmspaceSpawnEnv) -> Result<Tid, KError> {
        match vmspace::spawn_vmspace(self.id, env) {
            SyscallResult::Ok(tid) => Ok(tid),
            SyscallResult::Err(e) => Err(e),
        }
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
