// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::capabilities::{Capability, CapabilityRights};

pub struct Provider<'a> {
    fdt: &'a fdt::Fdt<'a>,
}

impl devicemgr::DevicemgrProvider for Provider<'_> {
    type Error = ();

    fn request(&mut self, compatible: Vec<String>) -> Result<Vec<devicemgr::Device>, Self::Error> {
        println!("Request for: {:?}", compatible);
        let mut devices = Vec::new();

        for compatible in self.fdt.all_nodes().filter_map(|n| {
            Some({
                n.compatible()?.all().find(|c| compatible.iter().any(|c2| c2 == c))?;
                n
            })
        }) {
            let cptr = librust::syscalls::io::claim_device(compatible.name).unwrap();
            devices.push(devicemgr::Device {
                name: compatible.name.into(),
                compatible: compatible.compatible().unwrap().all().map(ToString::to_string).collect(),
                interrupts: compatible.interrupts().map(|ints| ints.collect()).unwrap_or_default(),
                capability: Capability {
                    cptr,
                    rights: CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT,
                },
            });
        }

        Ok(devices)
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
    println!("[devicemgr] Running server");
    devicemgr::Devicemgr::new(Provider { fdt: &fdt }).serve();
}
