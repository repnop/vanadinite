// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    ipc::IpcChannel,
    librust::{
        message::KernelNotification,
        syscalls::{receive_message, ReadMessage},
    },
};

fn main() {
    println!("[servicemgr] I live! Trying to send a message to devicemgr with a cap we got from our parent");
    let mut msg = receive_message();

    while msg.is_none() {
        msg = receive_message();
    }

    let cap = match msg.unwrap() {
        ReadMessage::Kernel(KernelNotification::ChannelOpened(cap)) => cap,
        _ => unreachable!(),
    };

    let mut channel = IpcChannel::new(cap);
    let mut message = channel.new_message(4096).unwrap();
    message.write(b"hell yeah");
    message.send().unwrap();

    loop {}
}
