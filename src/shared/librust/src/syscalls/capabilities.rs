// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2023 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::CapabilityPtr,
    error::{RawSyscallError, SyscallError},
    syscalls::Syscall,
};

/// Delete a resource associated with the given [`CapabilityPtr`]. Attempting to
/// access a memory region which has been deallocated by this function will
/// cause undefined behavior.
///
/// ## Safety
///
/// Usage of this function can cause further attempts to access resources, such
/// as memory allocation, to result in undefined behavior. Care must be taken
/// when deleting a resource to ensure that it is not accessed after this is
/// called.
#[inline]
pub unsafe fn delete(cptr: CapabilityPtr) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::DeleteCapability as usize => error,
            in("a1") cptr.value(),
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}
