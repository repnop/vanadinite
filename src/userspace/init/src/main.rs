// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm, naked_functions, start, lang_items)]

mod rt_init;

use std::librust::{self, capabilities::CapabilityRights, syscalls::allocation::MemoryPermissions};

static mut FDT: *const u8 = core::ptr::null();
static SERVERS: &[u8] = include_bytes!("../../../../build/initfs.tar");

fn main() {
    let fdt = unsafe { fdt::Fdt::from_ptr(FDT).unwrap() };
    let fdt_size = fdt.total_size();
    let tar = tar::Archive::new(SERVERS).unwrap();

    println!("[INIT] args: {:?}", std::env::args());
    println!("[INIT] FDT @ {:#p}", unsafe { FDT });

    let devicemgr = tar.file("devicemgr").unwrap();
    let (space, mut env) = loadelf::load_elf(&loadelf::Elf::new(devicemgr.contents).unwrap()).unwrap();

    let mut fdt_obj = space.create_object(core::ptr::null(), fdt_size, MemoryPermissions::READ).unwrap();
    fdt_obj.as_slice()[..fdt_size].copy_from_slice(unsafe { core::slice::from_raw_parts(FDT, fdt_size) });

    let mut args = space.create_object(core::ptr::null(), 4096, MemoryPermissions::READ).unwrap();

    // This makes me sad, but seems to be the easiest approach..
    let ptr_str = format!("{:x}", fdt_obj.vmspace_address() as usize);
    let ptr_str_addr = args.vmspace_address() as usize + 16;
    args.as_slice()[..8].copy_from_slice(&ptr_str_addr.to_ne_bytes()[..]);
    args.as_slice()[8..16].copy_from_slice(&ptr_str.len().to_ne_bytes()[..]);
    args.as_slice()[16..][..ptr_str.len()].copy_from_slice(ptr_str.as_bytes());

    env.a0 = 1;
    env.a1 = args.vmspace_address() as usize;
    env.a2 = fdt_obj.vmspace_address() as usize;

    println!("[INIT] Spawning devicemgr");

    let (_, devicemgr_cptr) = space.spawn(env).unwrap();

    let message = librust::syscalls::channel::create_message(devicemgr_cptr, 12).unwrap();
    unsafe { core::slice::from_raw_parts_mut(message.ptr, message.len)[..12].copy_from_slice(&[b'A'; 12][..]) };
    librust::syscalls::channel::send_message(devicemgr_cptr, message.id, 12).unwrap();

    let servicemgr = tar.file("servicemgr").unwrap();
    let (space, mut env) = loadelf::load_elf(&loadelf::Elf::new(servicemgr.contents).unwrap()).unwrap();
    env.a0 = 0;

    println!("[INIT] Spawning servicemgr");

    let (_, servicemgr_cptr) = space.spawn(env).unwrap();
    librust::syscalls::channel::send_capability(
        devicemgr_cptr,
        servicemgr_cptr,
        CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT,
    )
    .unwrap();

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
