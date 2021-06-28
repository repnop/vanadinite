// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm, naked_functions, start, lang_items)]
#![no_std]

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    extern "C" {
        fn main(_: isize, _: *const *const u8) -> isize;
    }

    #[rustfmt::skip]
    asm!("
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop
    ");

    main(0, core::ptr::null::<*const u8>());
    librust::syscalls::exit()
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, _argc: isize, _argv: *const *const u8) -> isize {
    main();
    0
}
