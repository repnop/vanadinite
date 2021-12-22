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
    message::{Recipient, SyscallRequest, SyscallResult},
};

pub fn query_memory_capability(cptr: CapabilityPtr) -> SyscallResult<(*mut u8, usize, MemoryPermissions), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::QueryMemoryCapability,
            arguments: [cptr.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|(ptr, len, perms)| (ptr as *mut u8, len, MemoryPermissions::new(perms)))
}
