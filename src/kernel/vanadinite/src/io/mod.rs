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

use alloc::{collections::BTreeMap, string::String};
pub use console::*;
use core::fmt::Write;
use librust::task::Tid;

pub static CLAIMED_DEVICES: sync::SpinRwLock<BTreeMap<String, Tid>> = sync::SpinRwLock::new(BTreeMap::new());

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
