// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

mod client;

use filesystem::{
    block_devices::{BlockDevice, DeviceError},
    filesystems::Filesystem,
};
use librust::syscalls::io::query_mmio_cap;
use present::{
    futures::stream::{Stream, StreamExt},
    interrupt::Interrupt,
    ipc::NewChannelListener,
};
use std::sync::SyncRc;
use vidl::CapabilityPtr;

enum InitEvent {
    Event(Event),
    Filesystem(Result<Vec<SyncRc<dyn Filesystem>>, DeviceError>),
}

#[derive(Debug)]
enum Event {
    Interrupt { interrupt: usize, block_device_index: usize },
    NewChannel(CapabilityPtr),
}

#[present::main]
async fn main() {
    let virtiomgr = virtiomgr::VirtIoMgrClient::new(std::env::lookup_capability("virtiomgr").unwrap().capability.cptr);
    let devices = virtiomgr.request(virtio::DeviceType::BlockDevice as u32);
    if devices.is_empty() {
        return;
    }

    let mut block_devices = Vec::new();
    let mut interrupts = Box::new(present::futures::stream::pending()) as Box<dyn Stream<Item = Event> + Unpin>;
    let mut join_handles = Vec::new();

    for device in devices {
        let (mmio, _) = query_mmio_cap(device.capability.cptr, &mut []).unwrap();
        let virtio_device = filesystem::block_devices::virtio::VirtIoBlockDevice::new(unsafe {
            &*mmio.address().cast::<virtio::devices::block::VirtIoBlockDevice>()
        })
        .unwrap();

        let virtio_device = SyncRc::from_rc(std::rc::Rc::new(virtio_device) as std::rc::Rc<dyn BlockDevice>);
        let block_device_index = block_devices.len();

        for interrupt in device.interrupts {
            interrupts = Box::new(interrupts.merge(
                Interrupt::new(interrupt).map(move |interrupt| Event::Interrupt { interrupt, block_device_index }),
            ));
        }

        block_devices.push(SyncRc::clone(&virtio_device));
        join_handles.push(present::spawn(filesystem::probe::filesystem_probe(virtio_device)));
    }

    let join_handle_count = join_handles.len();
    let mut collected_handles = 0;
    let mut filesystems = Vec::new();
    let mut init_stream = Box::pin(interrupts.map(InitEvent::Event).merge(
        present::futures::stream::from_iter(join_handles).then(|h| Box::pin(h.join())).map(InitEvent::Filesystem),
    ));

    while let Some(event) = init_stream.next().await {
        match event {
            InitEvent::Event(Event::Interrupt { interrupt, block_device_index }) => {
                block_devices[block_device_index].handle_interrupt();
                librust::syscalls::io::complete_interrupt(interrupt).unwrap();
            }
            InitEvent::Filesystem(res) => {
                collected_handles += 1;
                match res {
                    Ok(fs) => filesystems.extend(fs),
                    Err(e) => println!("Error collecting filesystems for device: {e:?}"),
                }

                if collected_handles == join_handle_count {
                    break;
                }
            }
            _ => unreachable!(),
        }
    }

    let filesystems: SyncRc<[SyncRc<dyn Filesystem>]> = SyncRc::from(filesystems.into_boxed_slice());

    let (interrupts, _) = core::pin::Pin::into_inner(init_stream).unmerge();
    let event_stream = interrupts.into_stream().merge(NewChannelListener::new().map(Event::NewChannel));
    present::pin!(event_stream);

    while let Some(event) = event_stream.next().await {
        match event {
            Event::Interrupt { interrupt, block_device_index } => {
                block_devices[block_device_index].handle_interrupt();
                librust::syscalls::io::complete_interrupt(interrupt).unwrap();
            }
            Event::NewChannel(cptr) => {
                present::spawn(client::serve_client(cptr, SyncRc::clone(&filesystems)));
            }
        }
    }
}
