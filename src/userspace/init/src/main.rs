// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    self,
    capabilities::{CapabilityPtr, CapabilityRights},
    syscalls::mem::MemoryPermissions,
};

static SERVERS: &[u8] = include_bytes!("../../../../build/initfs.tar");

static INIT_ORDER: &[Service] = &[
    Service { name: "devicemgr", caps: &["fdt"] },
    Service { name: "stdio", caps: &["devicemgr"] },
    Service { name: "virtiomgr", caps: &["devicemgr", "stdio"] },
    Service { name: "filesystem", caps: &["virtiomgr", "stdio"] },
    // Service { name: "network", caps: &["virtiomgr", "stdio"] },
    // Service { name: "servicemgr", caps: &["devicemgr", "stdio"] },
    // Service { name: "echonet", caps: &["network", "stdio"] },
    Service { name: "fstest", caps: &["filesystem", "stdio"] },
];

struct Service {
    name: &'static str,
    caps: &'static [&'static str],
}

fn main() {
    let fdt_ptr = std::env::a2() as *const u8;
    let fdt = unsafe { fdt::Fdt::from_ptr(fdt_ptr).unwrap() };
    let fdt_size = fdt.total_size();
    let tar = tar::Archive::new(SERVERS).unwrap();

    let mut caps = std::collections::BTreeMap::<&'static str, CapabilityPtr>::new();

    for server in INIT_ORDER {
        let Some(file) = tar.file(server.name) else { panic!("Couldn't find service: {}", server.name) };
        let (mut space, mut env) = loadelf::load_elf(server.name, &loadelf::Elf::new(file.contents).unwrap()).unwrap();

        for cap in server.caps {
            if cap == &"fdt" {
                let mut fdt_obj = space.create_object(core::ptr::null(), fdt_size, MemoryPermissions::READ).unwrap();
                fdt_obj.as_slice()[..fdt_size]
                    .copy_from_slice(unsafe { core::slice::from_raw_parts(fdt_ptr, fdt_size) });
                env.a2 = fdt_obj.vmspace_address() as usize;
                continue;
            }

            let cptr = *caps.get(cap).unwrap();
            space.grant(cap, cptr, CapabilityRights::READ | CapabilityRights::WRITE);
        }

        env.a0 = 0;
        env.a1 = 0;

        let cap = space.spawn(env).unwrap();
        caps.insert(server.name, cap.get());
    }
}
