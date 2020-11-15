// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{SbiError, SbiResult};

pub const EXTENSION_ID: usize = 0x48534D;

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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HartStatus {
    Started,
    Stopped,
    StartRequestPending,
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
