// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod block_device;
pub mod console;
pub mod logging;
pub mod terminal;

pub use console::*;
use core::fmt::Write;
use crossbeam_queue::ArrayQueue;

pub static INPUT_QUEUE: crate::sync::Lazy<ArrayQueue<u8>> = crate::sync::Lazy::new(|| ArrayQueue::new(4096));

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => ($crate::print!("{}\r\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    CONSOLE.lock().write_fmt(args).unwrap();
}
