// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod ns16550;

use librust::{
    capabilities::{CapabilityDescription, CapabilityWithDescription},
    syscalls::channel::{ChannelMessage, KernelMessage},
};
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
    let mut devicemgr = std::ipc::IpcChannel::new(devicemgr.capability.cptr);

    let msg = &WantedCompatible { compatible: vec![String::from("ns16550"), String::from("ns16550a")] };
    devicemgr.temp_send_json(ChannelMessage::default(), msg, &[]).unwrap();

    let (_message, caps) = devicemgr.read_with_all_caps().unwrap();
    if caps.is_empty() {
        return;
    }

    let mut interrupt_buffer = [0];
    let (uart_info, _) =
        librust::syscalls::io::query_mmio_cap(caps[0].capability.cptr, &mut interrupt_buffer[..]).unwrap();

    let uart = unsafe { &*(uart_info.address() as *mut _ as *const Uart16550) };
    uart.init();

    // let devices: Devices = json::deserialize(message.as_bytes()).unwrap();

    // uart.write_str("Got the following devices from devicemgr:\n");
    // for device in devices.devices.iter() {
    //     uart.write_str(&format!("    {:?}\n", device));
    // }

    let mut input = Vec::new();
    loop {
        let cptr = match librust::syscalls::channel::read_kernel_message() {
            // hack to skip the notification from devicemgr since its
            // stale...
            KernelMessage::NewChannelMessage(cptr) if cptr.value() != 1 => cptr,
            KernelMessage::InterruptOccurred(id) => {
                let read = uart.read();
                librust::syscalls::io::complete_interrupt(id).unwrap();
                input.push(read);
                uart.write(read);
                continue;
            }
            _ => continue,
        };

        let msg = std::ipc::IpcChannel::new(cptr);
        let (msg, caps) = msg.read_with_all_caps().unwrap();

        if let Some(CapabilityWithDescription { description: CapabilityDescription::Memory { ptr, len, .. }, .. }) =
            caps.get(0)
        {
            for b in unsafe { core::slice::from_raw_parts(*ptr, (*len).min(msg.0[0])) } {
                uart.write(*b);
            }
        }
    }
}
