// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{mem::MemoryPermissions, Syscall};
use crate::{capabilities::CapabilityPtr, error::SyscallError, task::Tid};
use core::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VmspaceObjectId(usize);

impl VmspaceObjectId {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    pub const fn value(self) -> usize {
        self.0
    }
}

pub struct VmspaceObjectMapping {
    /// The aligned address for the
    pub address: *const u8,
    pub size: usize,
    pub permissions: MemoryPermissions,
}

pub fn create_vmspace() -> Result<VmspaceObjectId, SyscallError> {
    crate::syscalls::syscall(
        Recipient::kernel(),
        SyscallRequest { syscall: Syscall::CreateVmspace, arguments: [0; 12] },
    )
    .1
    .map(VmspaceObjectId)
}

pub fn alloc_vmspace_object(
    id: VmspaceObjectId,
    mapping: VmspaceObjectMapping,
) -> Result<(*mut u8, *mut u8), SyscallError> {
    crate::syscalls::syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::AllocVmspaceObject,
            arguments: [
                id.value(),
                mapping.address as usize,
                mapping.size,
                mapping.permissions.value(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        },
    )
    .1
}

pub struct VmspaceSpawnEnv {
    pub pc: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub sp: usize,
    pub tp: usize,
}

pub fn spawn_vmspace(
    id: VmspaceObjectId,
    name: &str,
    env: VmspaceSpawnEnv,
) -> Result<(Tid, CapabilityPtr), SyscallError> {
    crate::syscalls::syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::SpawnVmspace,
            arguments: [
                id.value(),
                name.as_ptr() as usize,
                name.len(),
                env.pc,
                env.a0,
                env.a1,
                env.a2,
                env.sp,
                env.tp,
                0,
                0,
                0,
            ],
        },
    )
    .1
    .map(|(n, cptr)| (Tid::new(NonZeroUsize::new(n).unwrap()), CapabilityPtr::new(cptr)))
}
