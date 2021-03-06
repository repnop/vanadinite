// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::librust::{
    self,
    message::SyscallResult,
    syscalls::{channel, ReadMessage},
};

fn main() {
    let mut channels = Vec::new();
    loop {
        let msg = librust::syscalls::receive_message();

        if let Some(ReadMessage::User(tid, msg)) = msg {
            //println!("\n[INIT] We received a message");

            if msg.contents[0] == 1 {
                //println!("[INIT] {:?} has asked us to open a channel!", tid);

                let channel_id = channel::create_channel(tid).unwrap();
                channels.push(channel_id);

                const HELLO_FRIEND: &str = "Hello, friend!";
                let cmsg = channel::create_message(channel_id, HELLO_FRIEND.len()).unwrap();
                let cmsg_slice = unsafe { core::slice::from_raw_parts_mut(cmsg.ptr, cmsg.len) };
                cmsg_slice[..HELLO_FRIEND.len()].copy_from_slice(HELLO_FRIEND.as_bytes());

                #[allow(clippy::drop_ref)]
                drop(cmsg_slice);

                channel::send_message(channel_id, cmsg.id, HELLO_FRIEND.len()).unwrap();

                //println!("[INIT] Sent a message!");
            }
        }

        for channel_id in &channels {
            match channel::read_message(*channel_id) {
                SyscallResult::Ok(Some(_)) => {} //println!("[INIT] Someone sent a message on {:?}", channel_id),
                SyscallResult::Ok(None) => {}
                SyscallResult::Err(_) => {} //println!("Error reading message from channel: {:?}", e),
            }
        }
    }
}
