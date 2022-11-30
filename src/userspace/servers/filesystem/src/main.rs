// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;
use filesystem::{
    block_devices::{BlockDevice, SectorIndex},
    partitions::{gpt::GptHeader, mbr::MasterBootRecord},
};
use librust::syscalls::io::query_mmio_cap;
use present::{futures::stream::StreamExt, interrupt::Interrupt};

enum Event {
    Interrupt(usize),
}

#[present::main]
async fn main() {
    // let mut block_devices = Vec::new();
    let virtiomgr = virtiomgr::VirtIoMgrClient::new(std::env::lookup_capability("virtiomgr").unwrap().capability.cptr);
    let mut devices = virtiomgr.request(virtio::DeviceType::BlockDevice as u32);
    if devices.is_empty() {
        return;
    }

    let device = devices.remove(0);
    let interrupt_id = device.interrupts[0];
    let (mmio, _) = query_mmio_cap(device.capability.cptr, &mut []).unwrap();
    let virtio_device = filesystem::block_devices::virtio::VirtIoBlockDevice::new(unsafe {
        &*mmio.address().cast::<virtio::devices::block::VirtIoBlockDevice>()
    })
    .unwrap();

    let mut mbr_response = Some(virtio_device.read(SectorIndex::new(0)));
    let mut gpt_response = Some(virtio_device.read(SectorIndex::new(1)));

    let stream = Interrupt::new(interrupt_id).map(Event::Interrupt);
    present::pin!(stream);

    while let Some(event) = stream.next().await {
        match event {
            Event::Interrupt(id) => {
                virtio_device.handle_interrupt();
                librust::syscalls::io::complete_interrupt(id).unwrap();

                if let Some(res) = mbr_response.take() {
                    let res = res.await.unwrap();
                    let mbr = MasterBootRecord::try_from_byte_slice(&res).unwrap();
                    println!("{mbr:#?}");
                    let res = gpt_response.take().unwrap().await.unwrap();
                    let mbr = GptHeader::try_from_byte_slice(&res).unwrap();
                    println!("{mbr:#?}");
                }
            }
        }
    }
}
