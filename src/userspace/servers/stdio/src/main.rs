// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod ns16550;

use librust::{message::KernelNotification, syscalls::ReadMessage};
use ns16550::Uart16550;

fn main() {
    let devicemgr = std::env::lookup_capability("devicemgr").unwrap();
    let mut devicemgr = std::ipc::IpcChannel::new(devicemgr);

    let msg = "ns16550,ns16550a";
    let mut message = devicemgr.new_message(msg.len()).unwrap();
    message.write(msg.as_bytes());
    message.send().unwrap();

    let response = devicemgr.read().unwrap();
    if response.as_bytes() != b"yes" {
        librust::syscalls::exit();
    }

    let uart_cap = devicemgr.receive_capability().unwrap();
    let (uart_ptr, _, _) = librust::syscalls::mem::query_memory_capability(uart_cap).unwrap();

    let uart = unsafe { &*(uart_ptr as *mut _ as *const Uart16550) };
    uart.init();

    uart.write_str("[stdio] got UART from devicemgr!\n");

    loop {
        let cptr = match librust::syscalls::receive_message() {
            ReadMessage::Kernel(kmsg) => match kmsg {
                KernelNotification::NewChannelMessage(cptr) => cptr,
                _ => continue,
            },
            _ => continue,
        };

        let msg = std::ipc::IpcChannel::new(cptr);
        let msg = msg.read().unwrap();

        for b in msg.as_bytes() {
            uart.write(*b);
        }
    }
}
