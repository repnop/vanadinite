// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod ns16550;

use librust::{message::KernelNotification, syscalls::ReadMessage};
use ns16550::Uart16550;

json::derive! {
    #[derive(Debug)]
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    #[derive(Debug)]
    struct Devices {
        devices: Vec<Device>,
    }
}

json::derive! {
    Serialize,
    struct WantedCompatible {
        compatible: Vec<String>,
    }
}

fn main() {
    let devicemgr = std::env::lookup_capability("devicemgr").unwrap();
    let mut devicemgr = std::ipc::IpcChannel::new(devicemgr);

    let msg = json::to_bytes(&WantedCompatible { compatible: vec![String::from("ns16550"), String::from("ns16550a")] });
    devicemgr.send_bytes(&msg, &[]).unwrap();

    let (_message, caps) = devicemgr.read_with_all_caps().unwrap();
    let uart_info = librust::syscalls::io::query_mmio_cap(caps[0].cptr).unwrap();

    let uart = unsafe { &*(uart_info.address() as *mut _ as *const Uart16550) };
    uart.init();

    // let devices: Devices = json::deserialize(message.as_bytes()).unwrap();

    // uart.write_str("Got the following devices from devicemgr:\n");
    // for device in devices.devices.iter() {
    //     uart.write_str(&format!("    {:?}\n", device));
    // }

    let mut input = Vec::new();
    loop {
        let cptr = match librust::syscalls::receive_message() {
            ReadMessage::Kernel(kmsg) => match kmsg {
                // hack to skip the notification from devicemgr since its
                // stale...
                KernelNotification::NewChannelMessage(cptr) if cptr.value() != 1 => cptr,
                KernelNotification::InterruptOccurred(id) => {
                    let read = uart.read();
                    librust::syscalls::io::complete_interrupt(id).unwrap();
                    input.push(read);
                    uart.write(read);
                    continue;
                }
                _ => continue,
            },
            _ => continue,
        };

        let msg = std::ipc::IpcChannel::new(cptr);
        let (msg, _) = msg.read_with_all_caps().unwrap();

        for b in msg.as_bytes() {
            uart.write(*b);
        }
    }
}
