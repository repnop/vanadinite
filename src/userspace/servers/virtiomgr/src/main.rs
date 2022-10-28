// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(drain_filter)]

use librust::{
    capabilities::{Capability, CapabilityRights, CapabilityWithDescription},
    syscalls::channel::KernelMessage,
};
use std::ipc::{ChannelMessage, ChannelReadFlags, IpcChannel};
use virtio::DeviceType;

json::derive! {
    #[derive(Debug, Clone)]
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    Deserialize,
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

json::derive! {
    Deserialize,
    struct VirtIoDeviceRequest {
        ty: u32,
    }
}

json::derive! {
    Serialize,
    struct VirtIoDeviceResponse {
        devices: Vec<Device>,
    }
}

fn main() {
    let devicemgr_cptr = std::env::lookup_capability("devicemgr").unwrap().capability.cptr;
    let devicemgr_client = devicemgr::DevicemgrClient::new(devicemgr_cptr);

    let devices = devicemgr_client.request(&["virtio,mmio"]);
    let _ = librust::syscalls::channel::read_kernel_message();
    let mut virtio_devices = Vec::new();

    for device in devices {
        // println!("{:?}", device);
        let (info, _) = librust::syscalls::io::query_mmio_cap(device.capability.cptr, &mut []).unwrap();

        let header = unsafe { &*(info.address() as *const virtio::VirtIoHeader) };
        let dev_type = header.device_type().unwrap();

        // if !matches!(dev_type, DeviceType::Reserved) {
        //     println!("[virtiomgr] We have a VirtIO {:?} device: {:?}", dev_type, device);
        // }

        virtio_devices.push((device.capability.cptr, info, dev_type, header, device));
    }

    loop {
        let cptr = match librust::syscalls::channel::read_kernel_message() {
            KernelMessage::NewChannelMessage(cptr) => cptr,
            _ => continue,
        };

        let channel = IpcChannel::new(cptr);
        let (req, _, _): (VirtIoDeviceRequest, _, _) = match channel.temp_read_json(ChannelReadFlags::NONBLOCKING) {
            Ok(data) => data,
            Err(_) => continue,
        };

        let dev_type = req.ty;

        println!("[virtiomgr] Got request for device type: {:?}", DeviceType::from_u32(dev_type));

        let devices: Vec<_> = virtio_devices.drain_filter(|device| device.2 as u32 == dev_type).collect();
        let caps: Vec<_> = devices
            .iter()
            .map(|(cap, _, _, _, _)| {
                Capability::new(*cap, CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT)
            })
            .collect();

        channel
            .temp_send_json(
                ChannelMessage::default(),
                &VirtIoDeviceResponse {
                    devices: devices
                        .iter()
                        .map(|dev| Device {
                            name: dev.4.name.clone(),
                            compatible: dev.4.compatible.clone(),
                            interrupts: dev.4.interrupts.clone(),
                        })
                        .collect(),
                },
                &caps,
            )
            .unwrap();
    }
}
