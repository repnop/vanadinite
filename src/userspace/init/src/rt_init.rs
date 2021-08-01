// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[no_mangle]
unsafe extern "C" fn _start(argc: isize, argv: *const *const u8, fdt: *const u8) -> ! {
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

    crate::FDT = fdt;
    main(argc, argv);
    std::librust::syscalls::exit()
}

extern "C" {
    static mut ARGS: [usize; 2];
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, argc: isize, argv: *const *const u8) -> isize {
    unsafe { ARGS = [argc as usize, argv as usize] };
    main();
    0
}
