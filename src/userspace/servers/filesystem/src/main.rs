// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod drivers;

use librust::{
    capabilities::{Capability, CapabilityPtr},
    message::KernelNotification,
    syscalls::ReadMessage,
};
use std::ipc::IpcChannel;

json::derive! {
    #[derive(Debug, Clone)]
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    Serialize,
    struct VirtIoDeviceRequest {
        ty: u32,
    }
}

json::derive! {
    Deserialize,
    struct VirtIoDeviceResponse {
        devices: Vec<Device>,
    }
}

struct BlockDevice {
    #[allow(dead_code)]
    mmio_cap: CapabilityPtr,
    #[allow(dead_code)]
    interrupts: Vec<usize>,
    device: drivers::virtio::BlockDevice,
}

fn main() {
    let mut block_devices = Vec::new();
    let mut virtiomgr = IpcChannel::new(std::env::lookup_capability("virtiomgr").unwrap());

    virtiomgr
        .send_bytes(&json::to_bytes(&VirtIoDeviceRequest { ty: virtio::DeviceType::BlockDevice as u32 }), &[])
        .unwrap();
    // println!("[filesystem] Sent device request");
    let (message, capabilities) = virtiomgr.read_with_all_caps().unwrap();
    let response: VirtIoDeviceResponse = json::deserialize(message.as_bytes()).unwrap();

    if response.devices.is_empty() {
        return;
    }

    for (Capability { cptr: mmio_cap, .. }, device) in capabilities.into_iter().zip(response.devices) {
        let info = librust::syscalls::io::query_mmio_cap(mmio_cap).unwrap();

        // println!("[filesystem] Got a VirtIO block device!");

        block_devices.push(BlockDevice {
            mmio_cap,
            interrupts: device.interrupts,
            device: drivers::virtio::BlockDevice::new(unsafe {
                &*(info.address() as *const virtio::devices::block::VirtIoBlockDevice)
            })
            .unwrap(),
        });
    }

    let drv = &mut block_devices[0].device;

    drv.queue_read(0);

    let id = loop {
        match librust::syscalls::receive_message() {
            ReadMessage::Kernel(KernelNotification::InterruptOccurred(id)) => {
                break id;
            }
            _ => continue,
        }
    };

    println!("[filesystem] Sector 0 = {:?}", drv.finish_command());
    librust::syscalls::io::complete_interrupt(id).unwrap();

    drv.queue_write(0, &[1; 512][..]);

    let id = loop {
        match librust::syscalls::receive_message() {
            ReadMessage::Kernel(KernelNotification::InterruptOccurred(id)) => {
                break id;
            }
            _ => continue,
        }
    };

    println!("[filesystem] Sector 0 = {:?}", drv.finish_command());
    librust::syscalls::io::complete_interrupt(id).unwrap();
}
