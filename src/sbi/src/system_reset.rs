// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{SbiError, SbiResult};

/// Timer extension ID
pub const EXTENSION_ID: usize = 0x53525354;

/// The type of reset to perform
#[derive(Debug, Clone, Copy)]
pub enum ResetType {
    /// Shutdown the system
    Shutdown,
    /// Power off all hardware and perform a cold boot
    ColdReboot,
    /// Reset processors and some hardware
    WarmReboot,
    /// Platform specific reset type
    PlatformSpecific(u32),
}

impl ResetType {
    /// Produce a `u32` from this `ResetType`
    pub fn to_u32(self) -> u32 {
        match self {
            ResetType::Shutdown => 0,
            ResetType::ColdReboot => 1,
            ResetType::WarmReboot => 2,
            ResetType::PlatformSpecific(n) => 0xF0000000 | (n & !0xF0000000),
        }
    }
}

/// The reason for performing the reset
#[derive(Debug, Clone, Copy)]
pub enum ResetReason {
    /// No reason for reset
    NoReason,
    /// System failure
    SystemFailure,
    /// SBI implementation specific reset reason
    SbiSpecific(u32),
    /// Platform specific reset reason
    PlatformSpecific(u32),
}

impl ResetReason {
    /// Produce a `u32` from this `ResetReason`
    pub fn to_u32(self) -> u32 {
        match self {
            ResetReason::NoReason => 0,
            ResetReason::SystemFailure => 1,
            ResetReason::SbiSpecific(n) => 0xE0000000 | (n & !0xE0000000),
            ResetReason::PlatformSpecific(n) => 0xF0000000 | (n & !0xF0000000),
        }
    }
}

/// Attempt to reset the system in the provided method, with a reason for the
/// reset.
pub fn system_reset(kind: ResetType, reason: ResetReason) -> SbiResult<!> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") kind.to_u32() as usize,
            in("a1") reason.to_u32() as usize,
            inout("a6") 0 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => unreachable!("this should not be possible on success"),
        e => SbiResult::Err(SbiError::new(e)),
    }
}
