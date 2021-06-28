// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

//! # `sbi`
//!
//! This crate implements an ergonomic interface to the RISC-V SBI
//! implementation that resides in M-mode

#![warn(missing_docs)]
#![feature(asm, never_type)]
#![no_std]

/// Required base SBI functionality
pub mod base;
/// Hart State Management extension
pub mod hart_state_management;
/// IPI extension
pub mod ipi;
/// Legacy SBI calls
pub mod legacy;
/// RFENCE extension
pub mod rfence;
/// System Reset extension
pub mod system_reset;
/// Timer extension
pub mod timer;

pub use base::{
    impl_id, impl_version, marchid, mimpid, mvendorid, probe_extension, spec_version, ExtensionAvailability,
};

/// Error codes returned by SBI calls
///
/// note: `SBI_SUCCESS` is not represented here since this is to be used as the
/// error type in a `Result`
#[derive(Debug, Clone, Copy)]
pub enum SbiError {
    /// The SBI call failed
    Failed,
    /// The SBI call is not implemented or the functionality is not available
    NotSupported,
    /// An invalid parameter was passed
    InvalidParam,
    /// The SBI implementation has denied execution of the call functionality
    Denied,
    /// An invalid address was passed
    InvalidAddress,
    /// The resource is already available
    AlreadyAvailable,
}

impl SbiError {
    #[inline]
    fn new(n: isize) -> Self {
        match n {
            -1 => SbiError::Failed,
            -2 => SbiError::NotSupported,
            -3 => SbiError::InvalidParam,
            -4 => SbiError::Denied,
            -5 => SbiError::InvalidAddress,
            -6 => SbiError::AlreadyAvailable,
            n => unreachable!("bad SBI error return value: {}", n),
        }
    }
}

/// The result of an SBI call
pub type SbiResult<T> = Result<T, SbiError>;

/// # Safety
/// This provides a
#[inline]
pub unsafe fn ecall(arguments: [usize; 6], extension_id: usize, function_id: usize) -> SbiResult<usize> {
    let error: isize;
    let value: usize;

    asm!(
        "ecall",
        in("a0") arguments[0],
        in("a1") arguments[1],
        inout("a2") arguments[2] => _,
        inout("a3") arguments[3] => _,
        inout("a4") arguments[4] => _,
        inout("a5") arguments[5] => _,
        inout("a6") function_id => _,
        inout("a7") extension_id => _,
        lateout("a0") error,
        lateout("a1") value,
    );

    match error {
        0 => SbiResult::Ok(value),
        e => SbiResult::Err(SbiError::new(e)),
    }
}
