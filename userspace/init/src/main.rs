// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm)]
#![no_std]

extern crate rt0;

fn main() {
    loop {
        let msg = librust::syscalls::receive_message();

        if let Ok(Some(_)) = msg {
            let _ = librust::syscalls::print(b"\n[INIT] We received a message");
        }
    }
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    librust::syscalls::exit()
}
