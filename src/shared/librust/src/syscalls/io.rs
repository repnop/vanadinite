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
};

/// Attempt to claim a device from the devicetree and receive an MMIO capability
/// to it.
///
/// FIXME: This should require a specific `SyscallCapability` of some kind
#[inline]
pub fn claim_device(node: &str) -> Result<CapabilityPtr, SyscallError> {
    let error: usize;
    let cptr: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ClaimDevice as usize => error,
            inlateout("a1") node.as_ptr() => cptr,
            in("a2") node.len(),
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok(CapabilityPtr::from_raw(cptr)),
    }
}

/// Mark a pending interrupt ID as having been serviced
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

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

unsafe impl Send for MmioCapabilityInfo {}
unsafe impl Sync for MmioCapabilityInfo {}

/// Information pertaining to a specific MMIO resource
pub struct MmioCapabilityInfo {
    address: *mut u8,
    len: usize,
    n_interrupts: usize,
}

impl MmioCapabilityInfo {
    /// Starting address of the MMIO region
    pub fn address(&self) -> *mut u8 {
        self.address
    }

    /// Length of the MMIO region
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Number of interrupts that the MMIO region has associated with it
    pub fn total_interrupts(&self) -> usize {
        self.n_interrupts
    }
}

/// Attempt to query an MMIO capability represented by the [`CapabilityPtr`],
/// returning information about the region and how many interrupts were read
/// into `interrupt_buffer`
#[inline]
pub fn query_mmio_cap(
    cptr: CapabilityPtr,
    interrupt_buffer: &mut [usize],
) -> Result<(MmioCapabilityInfo, usize), SyscallError> {
    let error: usize;
    let address: *mut u8;
    let len: usize;
    let n_interrupts: usize;
    let read_interrupts: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::QueryMmioCapability as usize => error,
            inlateout("a1") cptr.value() => address,
            inlateout("a2") interrupt_buffer.as_ptr() => len,
            inlateout("a3") interrupt_buffer.len() => n_interrupts,
            lateout("a4") read_interrupts,
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok((MmioCapabilityInfo { address, len, n_interrupts }, read_interrupts)),
    }
}

#[inline(never)]
#[no_mangle]
pub fn debug_print(value: &[u8]) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::DebugPrint as usize => error,
            in("a1") value.as_ptr(),
            in("a2") value.len(),
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}
