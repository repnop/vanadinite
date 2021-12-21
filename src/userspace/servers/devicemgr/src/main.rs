// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(lang_items)]

use librust::{capabilities::CapabilityRights, message::KernelNotification, syscalls::ReadMessage};
use std::ipc::IpcChannel;

fn main() {
    let args = std::env::args();
    let ptr = std::env::a2() as *const u8;
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

    loop {
        #[allow(clippy::collapsible_match)]
        let cptr = match librust::syscalls::receive_message() {
            ReadMessage::Kernel(kmsg) => match kmsg {
                KernelNotification::NewChannelMessage(cptr) => cptr,
                _ => continue,
            },
            _ => continue,
        };

        let mut channel = IpcChannel::new(cptr);
        let msg = channel.read().unwrap();
        let compatible: Vec<&str> = {
            let s = match core::str::from_utf8(msg.as_bytes()) {
                Ok(s) => s,
                Err(_) => continue,
            };

            s.split(',').collect()
        };

        match fdt.find_compatible(&compatible) {
            Some(device) => {
                let cptr = librust::syscalls::mem::claim_device(device.name).unwrap();
                channel
                    .send_bytes("yes")
                    .and_then(|_| {
                        channel.send_capability(
                            cptr,
                            CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT,
                        )
                    })
                    .unwrap();
            }
            None => {
                let _ = channel.send_bytes("no");
            }
        }
    }
}
