// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{SbiError, SbiResult};

/// Hart state management extension ID
pub const EXTENSION_ID: usize = 0x48534D;

/// Start the specific hart ID at the given physical address along with a
/// user-defined value. On success, the hart begins execution at the physical
/// address with the hart ID in `a0` and the user-defined value in `a1`, all
/// other register values are in an undefined state.
///
/// ## Possible errors
///
/// `InvalidAddress`: `start_address` is an invalid address because it is either
/// an invalid physical address or execution is prohibited by physical memory
/// protection
///
/// `InvalidParameter`: The specified hart ID is either not valid or cannot be
/// started in S-mode
///
/// `AlreadyAvailable`: The specified hart ID is already started
///
/// `Failed`: Start request failed for unknown reasons
pub fn hart_start(hart_id: usize, start_addr: usize, private: usize) -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_id,
            in("a1") start_addr,
            in("a2") private,
            inout("a6") 0 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
}

/// This SBI call stops S-mode execution on the current hart and yields
/// execution back to the SBI implementation. Note that this function must be
/// called with supervisor and user interrupts disabled.
///
/// ## Possible errors
///
/// `Failed`: The request failed for an unknown reason
pub fn hart_stop() -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            inout("a6") 1 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
}

/// Retrieve the status of the specified hart ID.
///
/// ## Possible errors
///
/// `InvalidParameter`: The specified hart ID is not valid
pub fn hart_status(hart_id: usize) -> SbiResult<HartStatus> {
    let value: usize;
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_id,
            inout("a6") 2 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
            lateout("a1") value,
        );
    }

    match error {
        0 => SbiResult::Ok(HartStatus::from_usize(value)),
        e => SbiResult::Err(SbiError::new(e)),
    }
}

/// Execution status for a hart
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HartStatus {
    /// The hart has already started execution
    Started,
    /// The hart is currently not active
    Stopped,
    /// A start request is pending for the hart
    StartRequestPending,
    /// A stop request is pending for the hart
    StopRequestPending,
}

impl HartStatus {
    fn from_usize(n: usize) -> Self {
        match n {
            0 => HartStatus::Started,
            1 => HartStatus::Stopped,
            2 => HartStatus::StartRequestPending,
            3 => HartStatus::StopRequestPending,
            _ => unreachable!("bad hart_status return value?"),
        }
    }
}
