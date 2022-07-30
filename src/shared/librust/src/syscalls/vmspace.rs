// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{mem::MemoryPermissions, Syscall};
use crate::{
    capabilities::CapabilityPtr,
    error::{RawSyscallError, SyscallError},
};

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
    let error: usize;
    let id: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::CreateVmspace as usize => error,
            lateout("a1") id,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(VmspaceObjectId::new(id)),
    }
}

pub fn alloc_vmspace_object(
    id: VmspaceObjectId,
    mapping: VmspaceObjectMapping,
) -> Result<(*mut u8, *mut u8), SyscallError> {
    let error: usize;
    let ours: *mut u8;
    let theirs: *mut u8;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocVmspaceObject as usize => error,
            inlateout("a1") id.value() => ours,
            inlateout("a2") mapping.address => theirs,
            in("a3") mapping.size,
            in("a4") mapping.permissions.value(),
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((ours, theirs)),
    }
}

pub struct VmspaceSpawnEnv {
    pub pc: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub sp: usize,
    pub tp: usize,
}

pub fn spawn_vmspace(id: VmspaceObjectId, name: &str, env: VmspaceSpawnEnv) -> Result<CapabilityPtr, SyscallError> {
    let error: usize;
    let cptr: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::SpawnVmspace as usize => error,
            inlateout("a1") id.value() => cptr,
            in("a2") name.as_ptr(),
            in("a3") name.len(),
            in("t0") env.pc,
            in("t1") env.a0,
            in("t2") env.a1,
            in("t3") env.a2,
            in("t4") env.sp,
            in("t5") env.tp,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(CapabilityPtr::new(cptr)),
    }
}
