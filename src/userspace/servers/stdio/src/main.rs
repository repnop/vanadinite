// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod ns16550;

use std::ipc::ChannelReadFlags;

use librust::{
    capabilities::{CapabilityDescription, CapabilityWithDescription},
    syscalls::endpoint::KernelMessage,
};
use ns16550::Uart16550;

fn main() {
    let devicemgr = std::env::lookup_capability("devicemgr").unwrap();
    let devicemgr = devicemgr::DevicemgrClient::new(devicemgr.capability.cptr);

    let devices = devicemgr.request(&["ns16550", "ns16550a"]);

    let mut interrupt_buffer = [0];
    let (uart_info, _) =
        librust::syscalls::io::query_mmio_cap(devices[0].capability.cptr, &mut interrupt_buffer[..]).unwrap();

    let uart = unsafe { &*(uart_info.address() as *mut _ as *const Uart16550) };
    uart.init();

    // let devices: Devices = json::deserialize(message.as_bytes()).unwrap();

    // uart.write_str("Got the following devices from devicemgr:\n");
    // for device in devices.devices.iter() {
    //     uart.write_str(&format!("    {:?}\n", device));
    // }

    let mut input = Vec::new();
    librust::syscalls::task::enable_notifications();
    loop {
        let cptr = match librust::syscalls::endpoint::read_kernel_message() {
            // hack to skip the notification from devicemgr since its
            // stale...
            KernelMessage::NewEndpointMessage(cptr) if cptr.value() != 1 => cptr,
            KernelMessage::InterruptOccurred(id) => {
                let read = uart.read();
                librust::syscalls::io::complete_interrupt(id).unwrap();
                input.push(read);
                uart.write(if read == b'\r' {
                    uart.write(b'\r');
                    b'\n'
                } else {
                    read
                });
                continue;
            }
            _ => continue,
        };

        let msg = std::ipc::IpcChannel::new(cptr);
        let (msg, caps) = match msg.read_with_all_caps(ChannelReadFlags::NONBLOCKING) {
            Ok(data) => data,
            Err(_) => continue,
        };

        if let Some(CapabilityWithDescription { description: CapabilityDescription::Memory { ptr, len, .. }, .. }) =
            caps.get(0)
        {
            for b in unsafe { core::slice::from_raw_parts(*ptr, (*len).min(msg.0[1])) } {
                uart.write(*b);
            }
        }
    }
}
