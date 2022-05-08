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

#[inline]
pub fn claim_device(node: &str) -> Result<CapabilityPtr, SyscallError> {
    let error: usize;
    let cptr: CapabilityPtr;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ClaimDevice as usize => error,
            inlateout("a1") node.as_ptr() => cptr,
            in("a2") node.len(),
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(cptr),
    }
}

#[inline]
pub fn complete_interrupt(interrupt_id: usize) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::CompleteInterrupt as usize => error,
            in("a1") interrupt_id,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

unsafe impl Send for MmioCapabilityInfo {}
unsafe impl Sync for MmioCapabilityInfo {}

pub struct MmioCapabilityInfo {
    address: *mut u8,
    len: usize,
    mem_perms: MemoryPermissions,
    n_interrupts: usize,
}

impl MmioCapabilityInfo {
    pub fn address(&self) -> *mut u8 {
        self.address
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn memory_permissions(&self) -> MemoryPermissions {
        self.mem_perms
    }

    pub fn total_interrupts(&self) -> usize {
        self.n_interrupts
    }
}

pub fn query_mmio_cap(
    cptr: CapabilityPtr,
    interrupt_buffer: &mut [usize],
) -> Result<(MmioCapabilityInfo, usize), SyscallError> {
    let error: usize;
    let address: *mut u8;
    let len: usize;
    let mem_perms: usize;
    let n_interrupts: usize;
    let read_interrupts: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::QueryMmioCapability => error,
            inlateout("a1") interrupt_buffer.as_ptr() => address,
            inlateout("a2") interrupt_buffer.len() => len,
            lateout("a3") mem_perms,
            lateout("a4") n_interrupts,
            lateout("a5") read_interrupts,
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok((
            MmioCapabilityInfo { address, len, mem_perms: MemoryPermissions::new(mem_perms), n_interrupts },
            read_interrupts,
        )),
    }
}

#[inline]
pub fn debug_print(value: &[u8]) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::DebugPrint => error,
            in("a1") value.as_ptr(),
            in("a2") value.len(),
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}
