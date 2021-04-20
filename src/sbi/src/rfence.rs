// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{ecall, SbiResult};

/// The RFENCE extension ID
pub const EXTENSION_ID: usize = 0x52464E43;

/// Instructs the given harts to execute a `FENCE.I` instruction
pub fn remote_fence_i(hart_mask: usize, hart_mask_base: usize) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, 0, 0, 0, 0], EXTENSION_ID, 0).map(drop) }
}

/// Instructs the given harts to execute a `SFENCE.VMA` for the region contained
/// by `start_addr` and `size`
pub fn remote_sfence_vma(hart_mask: usize, hart_mask_base: usize, start_addr: usize, size: usize) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, start_addr, size, 0, 0], EXTENSION_ID, 1).map(drop) }
}

/// Instructs the given harts to execute a `SFENCE.VMA` for the region contained
/// by `start_addr` and `size`, only covering the provided ASID
pub fn remote_sfence_vma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    asid: usize,
) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, start_addr, size, asid, 0], EXTENSION_ID, 2).map(drop) }
}

/// Instructs the given harts to execute a `HFENCE.GVMA` for the region
/// contained by `start_addr` and `size`, only covering the provided VMID. Only
/// valid on harts which support the hypervisor extension
pub fn remote_hfence_gvma_vmid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    vmid: usize,
) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, start_addr, size, vmid, 0], EXTENSION_ID, 3).map(drop) }
}

/// Instructs the given harts to execute a `HFENCE.GVMA` for the region
/// contained by `start_addr` and `size`. Only valid on harts which support the
/// hypervisor extension
pub fn remote_hfence_gvma(hart_mask: usize, hart_mask_base: usize, start_addr: usize, size: usize) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, start_addr, size, 0, 0], EXTENSION_ID, 4).map(drop) }
}

/// Instructs the given harts to execute a `HFENCE.VVMA` for the region
/// contained by `start_addr` and `size` for the current VMID of the calling
/// hart, and the given ASID. Only valid on harts which support the hypervisor
/// extension
pub fn remote_hfence_vvma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    asid: usize,
) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, start_addr, size, asid, 0], EXTENSION_ID, 5).map(drop) }
}

/// Instructs the given harts to execute a `HFENCE.VVMA` for the region
/// contained by `start_addr` and `size` for the current VMID of the calling
/// hart. Only valid on harts which support the hypervisor extension
pub fn remote_hfence_vvma(hart_mask: usize, hart_mask_base: usize, start_addr: usize, size: usize) -> SbiResult<()> {
    unsafe { ecall([hart_mask, hart_mask_base, start_addr, size, 0, 0], EXTENSION_ID, 6).map(drop) }
}
