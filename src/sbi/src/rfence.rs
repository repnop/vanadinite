// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{SbiError, SbiResult};

/// The RFENCE extension ID
pub const EXTENSION_ID: usize = 0x52464E43;

/// Instructs the given harts to execute a `FENCE.I` instruction
pub fn remote_fence_i(hart_mask: usize, hart_mask_base: usize) -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
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

/// Instructs the given harts to execute a `SFENCE.VMA` for the region contained
/// by `start_addr` and `size`
pub fn remote_sfence_vma(hart_mask: usize, hart_mask_base: usize, start_addr: usize, size: usize) -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") start_addr,
            in("a3") size,
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

/// Instructs the given harts to execute a `SFENCE.VMA` for the region contained
/// by `start_addr` and `size`, only covering the provided ASID
pub fn remote_sfence_vma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    asid: usize,
) -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") start_addr,
            in("a3") size,
            in("a4") asid,
            inout("a6") 2 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
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
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") start_addr,
            in("a3") size,
            in("a4") vmid,
            inout("a6") 3 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
}

/// Instructs the given harts to execute a `HFENCE.GVMA` for the region
/// contained by `start_addr` and `size`. Only valid on harts which support the
/// hypervisor extension
pub fn remote_hfence_gvma(hart_mask: usize, hart_mask_base: usize, start_addr: usize, size: usize) -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") start_addr,
            in("a3") size,
            inout("a6") 4 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
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
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") start_addr,
            in("a3") size,
            in("a4") asid,
            inout("a6") 5 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
}

/// Instructs the given harts to execute a `HFENCE.VVMA` for the region
/// contained by `start_addr` and `size` for the current VMID of the calling
/// hart. Only valid on harts which support the hypervisor extension
pub fn remote_hfence_vvma(hart_mask: usize, hart_mask_base: usize, start_addr: usize, size: usize) -> SbiResult<()> {
    let error: isize;

    unsafe {
        asm!(
            "ecall",
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") start_addr,
            in("a3") size,
            inout("a6") 6 => _,
            inout("a7") EXTENSION_ID => _,
            lateout("a0") error,
        );
    }

    match error {
        0 => SbiResult::Ok(()),
        e => SbiResult::Err(SbiError::new(e)),
    }
}
