// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::Syscall;
use crate::{
    capabilities::CapabilityPtr,
    error::{RawSyscallError, SyscallError},
    mem::PhysicalAddress,
    units::Bytes,
};

pub fn query_memory_capability(cptr: CapabilityPtr) -> Result<(*mut u8, usize, MemoryPermissions), SyscallError> {
    let error: usize;
    let virt: *mut u8;
    let size: usize;
    let perms: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::QueryMemoryCapability as usize => error,
            inlateout("a1") cptr.value() => virt,
            lateout("a2") size,
            lateout("a3") perms,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((virt, size, MemoryPermissions::new(perms))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryPermissions(usize);

impl MemoryPermissions {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXECUTE: Self = Self(1 << 2);

    pub const READ_WRITE: Self = Self(Self::READ.0 | Self::WRITE.0);

    pub fn new(flags: usize) -> Self {
        Self(flags)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for MemoryPermissions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for MemoryPermissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for MemoryPermissions {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.0 & rhs.0 == rhs.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct AllocationOptions(usize);

impl AllocationOptions {
    pub const NONE: Self = Self(0);
    pub const LARGE_PAGE: Self = Self(1 << 0);
    pub const ZERO: Self = Self(1 << 1);
    pub const ZERO_ON_DROP: Self = Self(1 << 2);
    pub const LAZY: Self = Self(1 << 3);
    pub const JOB_GROUP_AVAILABLE: Self = Self(1 << 4);
    pub const PRIVATE: Self = Self(1 << 5);

    pub fn new(flags: usize) -> Self {
        Self(flags)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for AllocationOptions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for AllocationOptions {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.0 & rhs.0 == rhs.0
    }
}

#[inline]
pub fn alloc_virtual_memory(
    size: Bytes,
    options: AllocationOptions,
    perms: MemoryPermissions,
) -> Result<(CapabilityPtr, *mut [u8]), SyscallError> {
    let error: usize;
    let virt: *mut u8;
    let real_size: usize;
    let cptr: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocVirtualMemory as usize => error,
            inlateout("a1") size.0 => cptr,
            inlateout("a2") options.0 => virt,
            inlateout("a3") perms.0 => real_size,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((CapabilityPtr::new(cptr), core::ptr::slice_from_raw_parts_mut(virt, real_size))),
    }
}

pub struct DmaAllocationOptions(usize);

impl DmaAllocationOptions {
    pub const NONE: Self = Self(0);
    pub const ZERO: Self = Self(1 << 1);

    pub fn new(flags: usize) -> Self {
        Self(flags)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for DmaAllocationOptions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for DmaAllocationOptions {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.0 & rhs.0 == rhs.0
    }
}

pub fn alloc_dma_memory(
    size_in_bytes: usize,
    options: DmaAllocationOptions,
) -> Result<(PhysicalAddress, *mut u8), SyscallError> {
    let error: usize;
    let phys: usize;
    let virt: *mut u8;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocDmaMemory as usize => error,
            inlateout("a1") size_in_bytes => phys,
            inlateout("a2") options.0 => virt,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((PhysicalAddress::new(phys), virt)),
    }
}
