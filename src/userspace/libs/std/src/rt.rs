// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    capabilities::{Capability, CapabilityDescription, CapabilityPtr, CapabilityRights, CapabilityWithDescription},
    syscalls::{
        channel::{ReadResult, PARENT_CHANNEL},
        mem::MemoryPermissions,
    },
};

#[no_mangle]
unsafe extern "C" fn _start(argc: isize, argv: *const *const u8, a2: usize) -> ! {
    extern "C" {
        fn main(_: isize, _: *const *const u8) -> isize;
    }

    #[rustfmt::skip]
    core::arch::asm!("
            .option push
            .option norelax
            lla gp, __global_pointer$
            .option pop

            lla {bss_start}, __bss_start
            lla {bss_end}, end
            1:
                sb zero, 0({bss_start})
                addi {bss_start}, {bss_start}, 1
                blt {bss_start}, {bss_end}, 1b
        ",
        bss_start = out(reg) _,
        bss_end = out(reg) _,
    );

    A2 = a2;

    main(argc, argv);
    librust::syscalls::task::exit()
}

extern "C" {
    static mut ARGS: [usize; 2];
    static mut A2: usize;
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, argc: isize, argv: *const *const u8) -> isize {
    unsafe { ARGS = [argc as usize, argv as usize] };

    let mut map = crate::env::CAP_MAP.borrow_mut();
    let channel = crate::ipc::IpcChannel::new(PARENT_CHANNEL);
    // FIXME: Wowie is this some awful code!
    if let Ok((names, _, caps)) = channel.temp_read_json::<Vec<String>>() {
        for (name, cap) in names.into_iter().zip(caps) {
            map.insert(name, cap);
        }
    }

    map.insert(
        "parent".into(),
        CapabilityWithDescription {
            capability: Capability { cptr: PARENT_CHANNEL, rights: CapabilityRights::READ | CapabilityRights::WRITE },
            description: CapabilityDescription::Channel,
        },
    );
    drop(map);

    main();
    0
}
