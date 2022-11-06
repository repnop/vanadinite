// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::syscalls::io::query_mmio_cap;

mod drivers;

fn main() {
    let mut block_devices = Vec::new();
    let virtiomgr = virtiomgr::VirtIoMgrClient::new(std::env::lookup_capability("virtiomgr").unwrap().capability.cptr);
    let devices = virtiomgr.request(virtio::DeviceType::BlockDevice as u32);

    for device in devices {
        let (mmio, _) = query_mmio_cap(device.capability.cptr, &mut []).unwrap();
        block_devices.push(
            drivers::virtio::BlockDevice::new(unsafe {
                &*mmio.address().cast::<virtio::devices::block::VirtIoBlockDevice>()
            })
            .unwrap(),
        );
    }

    block_devices[0].queue_read(0);

    loop {
        let notif = librust::syscalls::channel::read_kernel_message();
        match notif {
            vidl::internal::KernelMessage::InterruptOccurred(id) => {
                println!("[filesystem] {:?}", block_devices[0].finish_command().unwrap());
                librust::syscalls::io::complete_interrupt(id).unwrap();
            }
            _ => {}
        }
    }
}
