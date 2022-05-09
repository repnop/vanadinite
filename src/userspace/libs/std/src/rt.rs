// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::ipc::ReadChannelMessage;
use librust::capabilities::Capability;

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
    let channel = crate::ipc::IpcChannel::new(librust::capabilities::CapabilityPtr::new(0));
    let mut cap = [Capability::default()];

    // FIXME: Wowie is this some awful code!
    while let Ok(ReadChannelMessage { message: msg, .. }) = channel.read(&mut cap[..]) {
        let _ = librust::syscalls::receive_message();
        let name = match core::str::from_utf8(msg.as_bytes()) {
            Ok(name) => name,
            Err(_) => break,
        };

        if name == "done" {
            break;
        }

        map.insert(name.into(), cap[0].cptr);
    }

    map.insert("parent".into(), librust::capabilities::CapabilityPtr::new(0));
    drop(map);

    main();
    0
}
