// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(drain_filter)]

use librust::{
    capabilities::{Capability, CapabilityRights},
    message::KernelNotification,
    syscalls::ReadMessage,
};
use std::ipc::IpcChannel;
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
    let devicemgr_cptr = std::env::lookup_capability("devicemgr").unwrap();
    let mut devicemgr = IpcChannel::new(devicemgr_cptr);
    devicemgr.send_bytes(&json::to_bytes(&WantedCompatible { compatible: vec!["virtio,mmio".into()] }), &[]).unwrap();

    let (message, capabilities) = devicemgr.read_with_all_caps().unwrap();
    let devices: Devices = json::deserialize(message.as_bytes()).unwrap();
    let mut virtio_devices = Vec::new();

    for (device, Capability { cptr: mmio_cap, .. }) in devices.devices.into_iter().zip(capabilities) {
        let info = librust::syscalls::io::query_mmio_cap(mmio_cap).unwrap();

        let header = unsafe { &*(info.address() as *const virtio::VirtIoHeader) };
        let dev_type = header.device_type().unwrap();
        // println!("[virtiomgr] We have a VirtIO {:?} device: {:?}", dev_type, device);

        virtio_devices.push((mmio_cap, info, dev_type, header, device));
    }

    loop {
        #[allow(clippy::collapsible_match)]
        let cptr = match librust::syscalls::receive_message() {
            ReadMessage::Kernel(kmsg) => match kmsg {
                KernelNotification::NewChannelMessage(cptr) if cptr != devicemgr_cptr => cptr,
                _ => continue,
            },
            _ => continue,
        };

        let mut channel = IpcChannel::new(cptr);
        let (msg, _) = channel.read_with_all_caps().unwrap();
        let dev_type = json::deserialize::<VirtIoDeviceRequest>(msg.as_bytes()).unwrap().ty;

        // println!("[virtiomgr] Got request for device type: {:?}", DeviceType::from_u32(dev_type));

        let devices: Vec<_> = virtio_devices.drain_filter(|device| device.2 as u32 == dev_type).collect();
        let caps: Vec<_> = devices
            .iter()
            .map(|(cap, _, _, _, _)| {
                Capability::new(*cap, CapabilityRights::READ | CapabilityRights::WRITE | CapabilityRights::GRANT)
            })
            .collect();

        channel
            .send_bytes(
                &json::to_bytes(&VirtIoDeviceResponse { devices: devices.iter().map(|dev| dev.4.clone()).collect() }),
                &caps,
            )
            .unwrap();
    }
}
