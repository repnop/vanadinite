// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub fn pause() {
    #[cfg(feature = "sifive_u")]
    unsafe {
        core::arch::asm!(".word 0x0100000F")
    };
}

pub fn gp() -> *mut u8 {
    let gp: usize;
    unsafe { core::arch::asm!("mv {}, gp", out(reg) gp) };
    gp as *mut u8
}

pub fn ra() -> *mut u8 {
    let ra: usize;
    unsafe { core::arch::asm!("mv {}, ra", out(reg) ra) };
    ra as *mut u8
}
