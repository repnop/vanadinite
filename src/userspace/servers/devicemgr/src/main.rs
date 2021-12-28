// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(lang_items)]

use librust::{capabilities::CapabilityRights, message::KernelNotification, syscalls::ReadMessage};
use std::ipc::IpcChannel;

json::derive! {
    Serialize,
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    Serialize,
    struct Devices {
        devices: Vec<Device>,
    }
}

json::derive! {
    Deserialize,
    struct WantedCompatible {
        compatible: Vec<String>,
    }
}

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

    let mut buffer = Vec::new();
    loop {
        buffer.clear();

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
        let compatible = json::deserialize::<WantedCompatible>(msg.as_bytes()).unwrap().compatible;

        let all_compatible = fdt
            .all_nodes()
            .filter_map(|n| {
                Some({
                    n.compatible()?.all().find(|c| compatible.iter().any(|c2| c2 == c))?;
                    n
                })
            })
            .collect::<Vec<_>>();

        match all_compatible.len() {
            0 => drop(channel.send_bytes({
                json::serialize(&mut buffer, &Devices { devices: vec![] });
                &buffer
            })),
            _ => {
                let devices = Devices {
                    devices: all_compatible
                        .iter()
                        .map(|n| Device {
                            name: n.name.into(),
                            compatible: n.compatible().unwrap().all().map(ToString::to_string).collect(),
                            interrupts: n.interrupts().map(|ints| ints.collect()).unwrap_or_default(),
                        })
                        .collect(),
                };

                channel
                    .send_bytes({
                        json::serialize(&mut buffer, &devices);
                        &buffer
                    })
                    .unwrap();

                for device in all_compatible {
                    let cptr = librust::syscalls::io::claim_device(device.name).unwrap();
                    channel
                        .send_capability(
                            cptr,
                            CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT,
                        )
                        .unwrap();
                }
            }
        }
    }
}
