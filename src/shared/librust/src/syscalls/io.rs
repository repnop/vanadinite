// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{allocation::MemoryPermissions, syscall, Syscall};
use crate::{
    capabilities::CapabilityPtr,
    error::KError,
    message::{Message, Recipient, SyscallRequest, SyscallResult},
};

#[inline]
pub fn claim_device(node: &str) -> SyscallResult<CapabilityPtr, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::ClaimDevice,
            arguments: [node.as_ptr() as usize, node.len(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(CapabilityPtr::new)
}

#[inline]
pub fn complete_interrupt(interrupt_id: usize) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::CompleteInterrupt,
            arguments: [interrupt_id, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
}

unsafe impl Send for MmioCapabilityInfo {}
unsafe impl Sync for MmioCapabilityInfo {}

pub struct MmioCapabilityInfo {
    address: *mut u8,
    len: usize,
    mem_perms: MemoryPermissions,
    n_interrupts: usize,
    interrupts: [usize; 8],
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

    pub fn interrupts(&self) -> &[usize] {
        &self.interrupts[..self.n_interrupts]
    }
}

pub fn query_mmio_cap(cptr: CapabilityPtr) -> SyscallResult<MmioCapabilityInfo, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::QueryMmioCapability,
            arguments: [cptr.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|msg: Message| MmioCapabilityInfo {
        address: msg.contents[0] as *mut u8,
        len: msg.contents[1],
        mem_perms: MemoryPermissions::new(msg.contents[2]),
        n_interrupts: msg.contents[3],
        interrupts: msg.contents[4..12].try_into().unwrap(),
    })
}
