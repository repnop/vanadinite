// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(drain_filter)]

use virtio::DeviceType;

struct DiscoveredDevice {
    virtio_device_type: DeviceType,
    device: devicemgr::Device,
}

struct Provider {
    devices: Vec<DiscoveredDevice>,
}

impl virtiomgr::VirtIoMgrProvider for Provider {
    type Error = ();

    fn request(
        &mut self,
        virtio_device_type: vidl::core::U32,
    ) -> Result<vidl::core::Vec<devicemgr::Device>, Self::Error> {
        println!("[virtiomgr] Request for: {:?}", DeviceType::from_u32(virtio_device_type));
        match DeviceType::from_u32(virtio_device_type) {
            None => Ok(vec![]),
            Some(ty) => {
                Ok(self.devices.drain_filter(|dev| dev.virtio_device_type == ty).map(|dev| dev.device).collect())
            }
        }
    }
}

fn main() {
    let devicemgr_cptr = std::env::lookup_capability("devicemgr").unwrap().capability.cptr;
    let devicemgr_client = devicemgr::DevicemgrClient::new(devicemgr_cptr);

    let devices = devicemgr_client.request(&["virtio,mmio"]);
    let mut virtio_devices = Vec::new();

    for device in devices {
        // println!("{:?}", device);
        let (info, _) = librust::syscalls::io::query_mmio_cap(device.capability.cptr, &mut []).unwrap();

        let header = unsafe { &*(info.address() as *const virtio::VirtIoHeader) };
        let dev_type = header.device_type().unwrap();

        // if !matches!(dev_type, DeviceType::Reserved) {
        //     println!("[virtiomgr] We have a VirtIO {:?} device: {:?}", dev_type, device);
        // }

        virtio_devices.push(DiscoveredDevice { device, virtio_device_type: dev_type });
    }

    println!("[virtiomgr] Serving!");

    virtiomgr::VirtIoMgr::new(Provider { devices: virtio_devices }).serve();
}
