// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(
    allocator_api,
    alloc_error_handler,
    const_btree_new,
    extern_types,
    inline_const,
    lang_items,
    prelude_import,
    thread_local
)]
#![no_std]
#![allow(incomplete_features)]

extern crate alloc;

pub mod env;
pub mod heap;
pub mod io;
pub mod ipc;
pub mod prelude;
pub mod rc;
pub mod rt;
pub mod sync;
pub mod task;
mod task_local;
pub mod string {
    pub use alloc::string::*;
}
pub mod vec {
    pub use alloc::vec::*;
}
pub mod vmspace;

pub use alloc::collections;

#[prelude_import]
pub use prelude::rust_2021::*;

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
        $crate::println!("[{}:{}] {} = {:?}", file!(), line!(), stringify!($e), e);
        e
    }};
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    // FIXME: hack rn to make output less wacky
    // let out = args.to_string();
    // let _ = io::Stdout.write_str(&out);
    let _ = io::Stdout.write_fmt(args);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("PANIC: {}", info);
    librust::syscalls::task::exit()
}

#[alloc_error_handler]
fn alloc_error(layout: alloc::alloc::Layout) -> ! {
    panic!("Error allocating memory with layout: {:?}", layout)
}
