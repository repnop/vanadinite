// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(
    allocator_api,
    alloc_error_handler,
    coerce_unsized,
    extern_types,
    inline_const,
    lang_items,
    naked_functions,
    nonnull_slice_from_raw_parts,
    prelude_import,
    thread_local,
    unsize
)]
#![no_std]
#![allow(incomplete_features)]

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
pub mod alloc {
    extern crate alloc;
    pub use alloc::alloc::*;
}
pub mod collections {
    extern crate alloc;
    pub use alloc::collections::*;
}
pub mod string {
    extern crate alloc;
    pub use alloc::string::*;
}
pub mod vec {
    extern crate alloc;
    pub use alloc::vec::*;
}
pub mod vmspace;

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
fn alloc_error(layout: alloc::Layout) -> ! {
    panic!("Error allocating memory with layout: {:?}", layout)
}
