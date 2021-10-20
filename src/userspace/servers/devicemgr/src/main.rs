// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm, lang_items)]

use std::{ipc::IpcChannel, librust::capabilities::CapabilityPtr};

fn main() {
    let args = std::env::args();
    let ptr = usize::from_str_radix(args[0], 16).unwrap() as *const u8;
    println!("FDT is at: {:#p}", ptr);

    let fdt = unsafe { fdt::Fdt::from_ptr(ptr) }.unwrap();

    if args.contains(&"debug") {
        for node in fdt.all_nodes() {
            println!("{}: ", node.name);
            for prop in node.properties() {
                match &prop.value[..prop.value.len().max(1) - 1] {
                    s if s.iter().all(|b| b.is_ascii_graphic()) && !s.is_empty() => {
                        println!("    {}={}", prop.name, core::str::from_utf8(s).unwrap())
                    }
                    _ => println!("    {}={:?}", prop.name, prop.value),
                }
            }
        }
    }

    let (addr, _) = std::librust::syscalls::claim_device("/soc/uart").unwrap();

    println!("Claimed UART @ {:#p}!", addr);

    for i in 0..9 {
        unsafe { addr.write_volatile(i + b'0') };
    }

    unsafe { addr.write_volatile(b'\n') };

    let parent_channel = IpcChannel::new(CapabilityPtr::new(0));
    println!("{:?}", parent_channel.read());

    #[allow(clippy::empty_loop)]
    loop {}
}
