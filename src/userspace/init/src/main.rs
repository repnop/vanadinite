// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm, naked_functions, start, lang_items)]

mod rt_init;

use std::{
    ipc,
    librust::{
        self,
        message::KernelNotification,
        syscalls::{channel, ReadMessage},
    },
};

const HELLO_FRIEND: &str = "Hello, friend!";
static mut FDT: *const u8 = core::ptr::null();
static SERVERS: &[u8] = include_bytes!("../../../../initfs.tar");

fn main() {
    let fdt = unsafe { fdt::Fdt::from_ptr(FDT) };
    let tar = tar::Archive::new(SERVERS).unwrap();

    println!("[INIT] args: {:?}", std::env::args());
    println!("[INIT] FDT @ {:#p}", unsafe { FDT });
    println!("[INIT] Spawning shell");

    let shell = tar.file("shell").unwrap();
    let tid = loadelf::load_elf(&loadelf::Elf::new(shell.contents).unwrap()).unwrap();

    //let mut channels = Vec::new();
    //loop {
    //    let msg = librust::syscalls::receive_message();
    //
    //    if let Some(ReadMessage::Kernel(KernelNotification::ChannelRequest(tid))) = msg {
    //        let channel_id = channel::create_channel(tid).unwrap();
    //        let mut channel = ipc::IpcChannel::new(channel_id);
    //
    //        let mut msg = channel.new_message(HELLO_FRIEND.len()).unwrap();
    //        msg.write(HELLO_FRIEND.as_bytes());
    //        msg.send().unwrap();
    //
    //        channels.push(channel);
    //    }
    //
    //    for channel in &channels {
    //        match channel.read() {
    //            Ok(Some(_)) => {} //println!("[INIT] Someone sent a message on {:?}", channel_id),
    //            Ok(None) => {}
    //            Err(_) => {} //println!("Error reading message from channel: {:?}", e),
    //        }
    //    }
    //}

    loop {}
}
