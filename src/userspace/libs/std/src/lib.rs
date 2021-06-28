// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(allocator_api, alloc_error_handler, alloc_prelude, asm, inline_const, prelude_import, thread_local)]
#![no_std]
#![allow(incomplete_features)]

extern crate alloc;
extern crate rt0;

pub mod heap;
pub mod io;
pub mod prelude;
mod task_local;

pub use librust;

#[prelude_import]
pub use prelude::rust_2018::*;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => ($crate::print!("{}\r\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! dbg {
    ($e:expr) => {{
        let e = $e;
        println!("{} = {:?}", stringify!($e), $e);
        $e
    }};
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let _ = io::Stdout.write_fmt(args);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("PANIC: {}", info);
    librust::syscalls::exit()
}

#[alloc_error_handler]
fn alloc_error(layout: alloc::alloc::Layout) -> ! {
    panic!("Error allocating memory with layout: {:?}", layout)
}
