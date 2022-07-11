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
use std::ipc::{ChannelMessage, IpcChannel};
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
    let mut devicemgr = IpcChannel::new(devicemgr_cptr);
    devicemgr
        .temp_send_json(ChannelMessage::default(), &WantedCompatible { compatible: vec!["virtio,mmio".into()] }, &[])
        .unwrap();

    let (devices, _, capabilities): (Devices, _, _) = devicemgr.temp_read_json().unwrap();
    let mut virtio_devices = Vec::new();

    for (device, CapabilityWithDescription { capability: Capability { cptr: mmio_cap, .. }, .. }) in
        devices.devices.into_iter().zip(capabilities)
    {
        let (info, _) = librust::syscalls::io::query_mmio_cap(mmio_cap, &mut []).unwrap();

        let header = unsafe { &*(info.address() as *const virtio::VirtIoHeader) };
        let dev_type = header.device_type().unwrap();

        if !matches!(dev_type, DeviceType::Reserved) {
            println!("[virtiomgr] We have a VirtIO {:?} device: {:?}", dev_type, device);
        }

        virtio_devices.push((mmio_cap, info, dev_type, header, device));
    }

    loop {
        let cptr = match librust::syscalls::channel::read_kernel_message() {
            KernelMessage::NewChannelMessage(cptr) if cptr != devicemgr_cptr => cptr,
            _ => continue,
        };

        let channel = IpcChannel::new(cptr);
        let (req, _, _): (VirtIoDeviceRequest, _, _) = channel.temp_read_json().unwrap();
        let dev_type = req.ty;

        // println!("[virtiomgr] Got request for device type: {:?}", DeviceType::from_u32(dev_type));

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
                &VirtIoDeviceResponse { devices: devices.iter().map(|dev| dev.4.clone()).collect() },
                &caps,
            )
            .unwrap();
    }
}
