// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    self,
    capabilities::{CapabilityPtr, CapabilityRights},
    syscalls::allocation::MemoryPermissions,
};

static SERVERS: &[u8] = include_bytes!("../../../../build/initfs.tar");

static INIT_ORDER: &str = r#"{
    "servers": [
        {
            "name": "devicemgr",
            "caps": ["fdt"],
        },
        {
            "name": "stdio",
            "caps": ["devicemgr"],
        },
        {
            "name": "virtiomgr",
            "caps": ["devicemgr", "stdio"],
        },
        {
            "name": "filesystem",
            "caps": ["virtiomgr", "stdio"],
        },
        {
            "name": "network",
            "caps": ["virtiomgr", "stdio"],
        },
        {
            "name": "servicemgr",
            "caps": ["devicemgr", "stdio", "network"],
        },
    ]
}"#;

json::derive! {
    Deserialize,
    struct InitOrder {
        servers: Vec<Server>,
    }
}

json::derive! {
    Deserialize,
    struct Server {
        name: String,
        caps: Vec<String>,
    }
}

fn main() {
    let fdt_ptr = std::env::a2() as *const u8;
    let fdt = unsafe { fdt::Fdt::from_ptr(fdt_ptr).unwrap() };
    let fdt_size = fdt.total_size();
    let tar = tar::Archive::new(SERVERS).unwrap();

    let mut caps = std::collections::BTreeMap::<String, CapabilityPtr>::new();
    let init_order: InitOrder = json::deserialize(INIT_ORDER.as_bytes()).unwrap();

    for server in init_order.servers {
        let file = tar.file(&server.name).unwrap();
        let (mut space, mut env) = loadelf::load_elf(&server.name, &loadelf::Elf::new(file.contents).unwrap()).unwrap();

        for cap in server.caps {
            if cap == "fdt" {
                let mut fdt_obj = space.create_object(core::ptr::null(), fdt_size, MemoryPermissions::READ).unwrap();
                fdt_obj.as_slice()[..fdt_size]
                    .copy_from_slice(unsafe { core::slice::from_raw_parts(fdt_ptr, fdt_size) });
                env.a2 = fdt_obj.vmspace_address() as usize;
                continue;
            }

            let cptr = *caps.get(&cap).unwrap();
            space.grant(&cap, cptr, CapabilityRights::READ | CapabilityRights::WRITE);
        }

        env.a0 = 0;
        env.a1 = 0;
        let (_, cap) = space.spawn(env).unwrap();
        caps.insert(server.name, cap);
    }
}
