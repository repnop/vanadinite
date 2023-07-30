// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ptr::NonNull;

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

    // FIXME: turn this back to the clean way once const traits are usable again
    pub const READ_WRITE: Self = Self(Self::READ.0 | Self::WRITE.0);
    pub const RWX: Self = Self(Self::READ.0 | Self::WRITE.0 | Self::EXECUTE.0);

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

/// Attempt to allocate a region of virtual memory with the given size, options,
/// and permissions. A [`CapabilityPtr`] and slice pointer are returned,
/// allowing the region to be dellocated and shared between processes if
/// desired.
#[inline]
pub fn allocate_virtual_memory(size: Bytes, perms: MemoryPermissions) -> Result<*mut [u8], SyscallError> {
    let error: usize;
    let virt: *mut u8;
    let real_size: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocateVirtualMemory as usize => error,
            inlateout("a1") size.0 => virt,
            inlateout("a2") perms.0 => real_size,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(core::ptr::slice_from_raw_parts_mut(virt, real_size)),
    }
}

/// Deallocate a region of private virtual memory
///
/// ## Safety
///
/// The memory region specified by the address in `at` must not be accessed
/// after calling this function, otherwise accesses will result in undefined
/// behavior, and likely a fault, causing termination of the process.
#[inline]
pub unsafe fn deallocate_virtual_memory(at: *mut u8) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocateVirtualMemory as usize => error,
            in("a1") at,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

#[inline]
pub fn allocate_shared_memory(
    size: Bytes,
    perms: MemoryPermissions,
) -> Result<(CapabilityPtr, *mut [u8]), SyscallError> {
    let error: usize;
    let virt: *mut u8;
    let real_size: usize;
    let cptr: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocateSharedMemory as usize => error,
            inlateout("a1") size.0 => cptr,
            inlateout("a2") perms.0 => virt,
            lateout("a3") real_size,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((CapabilityPtr::new(cptr), core::ptr::slice_from_raw_parts_mut(virt, real_size))),
    }
}

/// Allocation options when attempting to allocate a region of
/// device-addressable memory
pub struct DmaAllocationOptions(usize);

impl DmaAllocationOptions {
    /// No flags
    pub const NONE: Self = Self(0);
    /// Zero the memory before receiving it
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

/// Attempt to allocate a region of memory, which the physical address of can be
/// given to devices.
///
/// FIXME: This should require a `SyscallCapability` or some such, and
/// eventually be removed in favor of an IOMMU-based approach
#[inline]
pub fn allocate_device_addressable_memory(
    size: Bytes,
    options: DmaAllocationOptions,
) -> Result<(PhysicalAddress, NonNull<u8>), SyscallError> {
    let error: usize;
    let phys: usize;
    let virt: *mut u8;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::AllocateDeviceAddressableMemory as usize => error,
            inlateout("a1") size.0 => phys,
            inlateout("a2") options.0 => virt,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((PhysicalAddress::new(phys), unsafe { NonNull::new_unchecked(virt) })),
    }
}
