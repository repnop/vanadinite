// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod drivers;

use librust::capabilities::{Capability, CapabilityPtr};
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
    // let mut block_devices = Vec::new();
    // let mut virtiomgr = IpcChannel::new(std::env::lookup_capability("virtiomgr").unwrap().capability.cptr);

    // virtiomgr
    //     .send_bytes(&json::to_bytes(&VirtIoDeviceRequest { ty: virtio::DeviceType::BlockDevice as u32 }), &[])
    //     .unwrap();
    // // println!("[filesystem] Sent device request");
    // let (message, capabilities) = virtiomgr.read_with_all_caps().unwrap();
    // let response: VirtIoDeviceResponse = json::deserialize(message.as_bytes()).unwrap();
}
