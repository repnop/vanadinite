// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm, naked_functions, start, lang_items)]

use librust::{self, capabilities::CapabilityRights, syscalls::allocation::MemoryPermissions};

static SERVERS: &[u8] = include_bytes!("../../../../build/initfs.tar");

fn main() {
    let fdt_ptr = std::env::a2() as *const u8;
    let fdt = unsafe { fdt::Fdt::from_ptr(fdt_ptr).unwrap() };
    let fdt_size = fdt.total_size();
    let tar = tar::Archive::new(SERVERS).unwrap();

    println!("[INIT] args: {:?}", std::env::args());
    println!("[INIT] fdt_ptr @ {:#p}", fdt_ptr);

    let servicemgr = tar.file("servicemgr").unwrap();
    let (space, mut env) = loadelf::load_elf(&loadelf::Elf::new(servicemgr.contents).unwrap()).unwrap();
    env.a0 = 0;

    println!("[INIT] Spawning servicemgr");

    let (_, servicemgr_cptr) = space.spawn(env).unwrap();

    let devicemgr = tar.file("devicemgr").unwrap();
    let (mut space, mut env) = loadelf::load_elf(&loadelf::Elf::new(devicemgr.contents).unwrap()).unwrap();

    let mut fdt_obj = space.create_object(core::ptr::null(), fdt_size, MemoryPermissions::READ).unwrap();
    fdt_obj.as_slice()[..fdt_size].copy_from_slice(unsafe { core::slice::from_raw_parts(fdt_ptr, fdt_size) });

    env.a0 = 0;
    env.a1 = 0;
    env.a2 = fdt_obj.vmspace_address() as usize;

    space.grant(
        "servicemgr",
        servicemgr_cptr,
        CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT,
    );

    println!("[INIT] Spawning devicemgr");

    let (_, devicemgr_cptr) = space.spawn(env).unwrap();

    let message = librust::syscalls::channel::create_message(devicemgr_cptr, 12).unwrap();
    unsafe { core::slice::from_raw_parts_mut(message.ptr, message.len)[..12].copy_from_slice(&[b'A'; 12][..]) };
    librust::syscalls::channel::send_message(devicemgr_cptr, message.id, 12).unwrap();

    loop {
        unsafe { asm!("nop") };
    }

    // println!("[INIT] Spawning shell");
    //
    // let shell = tar.file("shell").unwrap();
    // let (space, env) = loadelf::load_elf(&loadelf::Elf::new(shell.contents).unwrap()).unwrap();
    //
    // space.spawn(env).unwrap();

    // let mut channels = Vec::new();
    // loop {
    //     let msg = librust::syscalls::receive_message();
    //
    //     if let Some(ReadMessage::Kernel(KernelNotification::ChannelRequest(tid))) = msg {
    //         let channel_id = channel::create_channel(tid).unwrap();
    //         let mut channel = ipc::IpcChannel::new(channel_id);
    //
    //         let mut msg = channel.new_message(HELLO_FRIEND.len()).unwrap();
    //         msg.write(HELLO_FRIEND.as_bytes());
    //         msg.send().unwrap();
    //
    //         channels.push(channel);
    //     }
    //
    //     for channel in &channels {
    //         match channel.read() {
    //             Ok(Some(_)) => {} //println!("[INIT] Someone sent a message on {:?}", channel_id),
    //             Ok(None) => {}
    //             Err(_) => {} //println!("Error reading message from channel: {:?}", e),
    //         }
    //     }
    // }
}
