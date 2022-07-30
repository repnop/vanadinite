// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::{Capability, CapabilityDescription, CapabilityRights},
    syscalls::channel::{ChannelMessage, KernelMessage},
};
use std::ipc::{ChannelReadFlags, IpcChannel};

json::derive! {
    Serialize,
    #[derive(Debug)]
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    Serialize,
    #[derive(Debug)]
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
    librust::syscalls::task::enable_notifications();
    loop {
        // println!("[devicemgr] Waiting for new kernel message");
        let cptr = match librust::syscalls::channel::read_kernel_message() {
            KernelMessage::NewChannelMessage(cptr) => cptr,
            _ => continue,
        };

        // println!("[devicemgr] New channel message on {cptr:?}");

        let channel = IpcChannel::new(cptr);
        let (_, caps) = match channel.read_with_all_caps(ChannelReadFlags::NONBLOCKING) {
            Ok(data) => data,
            Err(_) => continue,
        };

        let mem = match &caps[0].description {
            CapabilityDescription::Memory { ptr, len, permissions: _ } => unsafe {
                core::slice::from_raw_parts(*ptr, *len)
            },
            _ => continue,
        };

        let compatible = json::deserialize::<WantedCompatible>(mem).unwrap().compatible;

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
            0 => channel.temp_send_json(ChannelMessage::default(), &Devices { devices: vec![] }, &[]).unwrap(),
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

                let mut caps = Vec::with_capacity(devices.devices.len());
                for device in all_compatible {
                    let cptr = librust::syscalls::io::claim_device(device.name).unwrap();
                    caps.push(Capability::new(
                        cptr,
                        CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT,
                    ));
                }

                channel.temp_send_json(ChannelMessage::default(), &devices, &caps[..]).unwrap();
            }
        }
    }
}
