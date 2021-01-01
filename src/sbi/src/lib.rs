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
