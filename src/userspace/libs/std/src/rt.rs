// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::{Capability, CapabilityRights},
    syscalls::endpoint::{ChannelReadFlags, IpcMessage, PARENT_CHANNEL},
};

#[naked]
#[no_mangle]
#[cfg_attr(feature = "init", link_section = ".rt.entry")]
unsafe extern "C" fn _start() -> ! {
    #[rustfmt::skip]
    core::arch::asm!("
            .option push
            .option norelax
            lla gp, __global_pointer$
            .option pop

            lla t0, __bss_start
            lla t1, end
            1:
                sb zero, 0(t0)
                addi t0, t0, 1
                blt t0, t1, 1b
            j _rust_start
        ",
        options(noreturn),
    );
}

#[no_mangle]
#[cfg_attr(feature = "init", link_section = ".rt.entry")]
unsafe extern "C" fn _rust_start(argc: isize, argv: *const *const u8, a2: usize) -> ! {
    extern "C" {
        fn main(_: isize, _: *const *const u8) -> isize;
    }

    A2 = a2;

    main(argc, argv);
    librust::syscalls::task::exit()
}

extern "C" {
    static mut ARGS: [usize; 2];
    static mut A2: usize;
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, argc: isize, argv: *const *const u8, _: u8) -> isize {
    unsafe { ARGS = [argc as usize, argv as usize] };

    let mut map = crate::env::CAP_MAP.borrow_mut();

    // FIXME: This is an inlined version of temp_read_json, replace this!
    let Ok(IpcMessage { identifier, message, capability, .. }) = crate::ipc::recv(ChannelReadFlags::NONE) else {
        panic!("unable to read initial spawn message");
    };

    if let Some(capability) = capability {
        let mut names = match capability.cptr.get_memory_region() {
            Some(region) if capability.rights & CapabilityRights::READ => {
                json::deserialize::<Vec<String>>(unsafe { &*region })
                    .expect("failed to deserialize JSON in channel message")
                    .into_iter()
            }
            _ => panic!("no or invalid mem cap"),
        };

        for _ in 0..message.0[0] {
            if let Ok(IpcMessage { identifier: nid, capability, .. }) = crate::ipc::recv(ChannelReadFlags::NONE) {
                assert_eq!(identifier, nid, "heck");

                match names.next().zip(capability) {
                    Some((name, cap)) => {
                        map.insert(name, cap);
                    }
                    None => break,
                }
            }
        }

        map.insert(
            "parent".into(),
            Capability { cptr: PARENT_CHANNEL.get(), rights: CapabilityRights::READ | CapabilityRights::WRITE },
        );
        drop(map);
    }

    main();
    0
}
