// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{ecall, SbiResult};

/// The IPI extension ID
pub const EXTENSION_ID: usize = 0x735049;

/// Send an inter-processor interrupt to the harts defined in `hart_mask`,
/// starting at `hart_mask_base`. The IPI is received on a hart as a supervisor
/// software interrupt.
pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, 0, 0, 0, 0], EXTENSION_ID, 0).map(drop) }
}
