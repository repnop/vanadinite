// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

/// `sbi_set_timer` extension ID
pub const SET_TIMER_EID: usize = 0x00;
/// yes
pub fn set_timer(stime: u64) {
    unsafe {
        asm!(
            "ecall",
            in("a0") stime,
            inout("a7") SET_TIMER_EID => _,
        );
    }
}

/// `sbi_console_putchar` extension ID
pub const CONSOLE_PUTCHAR_EID: usize = 0x01;
/// yes
pub fn console_putchar(c: u8) {
    unsafe {
        asm!(
            "ecall",
            in("a0") c as usize,
            inout("a7") CONSOLE_PUTCHAR_EID => _,
        );
    }
}

/// `sbi_console_getchar` extension ID
pub const CONSOLE_GETCHAR_EID: usize = 0x02;
/// yes
pub fn console_getchar() -> i8 {
    let mut ret: i8;

    unsafe {
        asm!(
            "ecall",
            lateout("a0") ret,
            inout("a7") CONSOLE_GETCHAR_EID => _,
        );
    }

    ret
}

// TODO: rest of legacy extension and fix docs
