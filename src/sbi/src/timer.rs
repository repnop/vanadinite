// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{ecall, SbiResult};

/// Timer extension ID
pub const EXTENSION_ID: usize = 0x54494D45;

/// Schedule an interrupt for `time` in the future. To clear the timer interrupt
/// without scheduling another timer event, a time infinitely far into the
/// future (`u64::MAX`) or mask the `STIE` bit of the `sie` CSR.
pub fn set_timer(time: u64) -> SbiResult<()> {
    unsafe { ecall([time as usize, 0, 0, 0, 0, 0], EXTENSION_ID, 0).map(drop) }
}
