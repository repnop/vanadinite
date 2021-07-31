// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{syscall, Syscall};
use crate::{
    error::KError,
    mem::PhysicalAddress,
    message::{Recipient, SyscallRequest, SyscallResult},
};

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MemoryPermissions(usize);

impl MemoryPermissions {
    pub const READ: Self = Self(0);
    pub const WRITE: Self = Self(1);
    pub const EXECUTE: Self = Self(2);

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

impl core::ops::BitAnd for MemoryPermissions {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.0 & rhs.0 == rhs.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct AllocationOptions(usize);

#[allow(non_upper_case_globals)]
impl AllocationOptions {
    pub const None: Self = Self(0);
    pub const LargePage: Self = Self(1 << 0);
    pub const Zero: Self = Self(1 << 1);
    pub const ZeroOnDrop: Self = Self(1 << 2);
    pub const Lazy: Self = Self(1 << 3);
    pub const JobGroupAvailable: Self = Self(1 << 4);

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
    size_in_bytes: usize,
    options: AllocationOptions,
    perms: MemoryPermissions,
) -> SyscallResult<*mut u8, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::AllocVirtualMemory,
            arguments: [size_in_bytes, options.value(), perms.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
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
) -> SyscallResult<(PhysicalAddress, *mut u8), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::AllocDmaMemory,
            arguments: [size_in_bytes, options.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|(phys, virt)| (PhysicalAddress::new(phys), virt as *mut u8))
}
