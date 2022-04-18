// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;

pub const INSUFFICIENT_RIGHTS: usize = 1;
pub const INVALID_OPERATION: usize = 2;
pub const INVALID_ARGUMENT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyscallError {
    InsufficientRights(u32),
    InvalidOperation(u32),
    InvalidArgument(u32),
}

impl SyscallError {
    pub const fn uncook(self) -> RawSyscallError {
        match self {
            Self::InsufficientRights(n) => RawSyscallError::new(NonZeroUsize::new(((n as usize) << 8) | INSUFFICIENT_RIGHTS).unwrap()),
            Self::InvalidOperation(n) => RawSyscallError::new(NonZeroUsize::new(((n as usize) << 8) | INVALID_OPERATION).unwrap()),
            Self::InvalidArgument(n) => RawSyscallError::new(NonZeroUsize::new(((n as usize) << 8) | INVALID_ARGUMENT).unwrap()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RawSyscallError(NonZeroUsize);

impl RawSyscallError {
    pub const fn new(value: NonZeroUsize) -> Self {
        Self(value)
    }

    pub const fn kind(self) -> usize {
        self.0.get() & 0xFF
    }

    pub const fn context(self) -> usize {
        self.0.get() >> 8
    }

    pub const fn cook(self) -> SyscallError {
        match self.kind() {
            INSUFFICIENT_RIGHTS => SyscallError::InsufficientRights(self.context() as u32),
            INVALID_OPERATION => SyscallError::InvalidOperation(self.context() as u32),
            INVALID_ARGUMENT => SyscallError::InvalidArgument(self.context() as u32),
            kind => unreachable!("invalid syscall error kind: {}", kind),
        }
    }
}
