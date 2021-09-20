// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod extensions;

// type Result<T> = core::result::Result<T, SbiError>;

/// SBI error codes
#[derive(Debug, Clone, Copy)]
#[repr(isize)]
pub enum SbiError {
    /// The SBI call failed
    Failed = -1,
    /// The SBI call is not implemented or the functionality is not available
    NotSupported = -2,
    /// An invalid parameter was passed
    InvalidParam = -3,
    /// The SBI implementation has denied execution of the call functionality
    Denied = -4,
    /// An invalid address was passed
    InvalidAddress = -5,
    /// The resource is already available
    AlreadyAvailable = -6,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SbiRet {
    pub error: isize,
    pub value: usize,
}
