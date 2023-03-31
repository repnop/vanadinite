// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

static SERVERS: &[u8] = include_bytes!("../../../../build/initfs.tar");

static INIT_ORDER: &[Service] = &[
    Service { name: "devicemgr", caps: &["fdt"] },
    Service { name: "stdio", caps: &["devicemgr"] },
    Service { name: "virtiomgr", caps: &["devicemgr", "stdio"] },
    Service { name: "filesystem", caps: &["virtiomgr", "stdio"] },
    // Service { name: "network", caps: &["virtiomgr", "stdio"] },
    // Service { name: "servicemgr", caps: &["devicemgr", "stdio"] },
    // Service { name: "echonet", caps: &["network", "stdio"] },
    Service { name: "fstest", caps: &["filesystem", "stdio"] },
];

struct Service {
    name: &'static str,
    caps: &'static [&'static str],
}

// #[present::main]
// async fn main() {
//     let fdt_ptr = std::env::a2() as *const u8;
//     let fdt = unsafe { fdt::Fdt::from_ptr(fdt_ptr).unwrap() };
//     let fdt_size = fdt.total_size();
//     let tar = tar::Archive::new(SERVERS).unwrap();

//     let mut caps = std::collections::BTreeMap::<&'static str, CapabilityPtr>::new();

//     for server in INIT_ORDER {
//         let Some(file) = tar.file(server.name) else { panic!("Couldn't find service: {}", server.name) };
//         let (mut space, mut env) = loadelf::load_elf(server.name, &loadelf::Elf::new(file.contents).unwrap()).unwrap();

//         for cap in server.caps {
//             if cap == &"fdt" {
//                 let mut fdt_obj = space.create_object(core::ptr::null(), fdt_size, MemoryPermissions::READ).unwrap();
//                 fdt_obj.as_slice()[..fdt_size]
//                     .copy_from_slice(unsafe { core::slice::from_raw_parts(fdt_ptr, fdt_size) });
//                 env.a2 = fdt_obj.vmspace_address() as usize;
//                 continue;
//             }

//             let cptr = *caps.get(cap).unwrap();
//             space.grant(cap, cptr, CapabilityRights::READ | CapabilityRights::WRITE);
//         }

//         env.a0 = 0;
//         env.a1 = 0;

//         let cap = space.spawn(env).unwrap();
//         caps.insert(server.name, cap);
//     }
// }

use filesystem::{
    block_devices::{BlockDevice, DeviceError},
    filesystems::{path::Path, FileId, FilePermissions, FileType, Filesystem, Root},
};
use librust::syscalls::io::query_mmio_cap;
use present::{
    futures::stream::{IntoStream, Stream, StreamExt},
    interrupt::Interrupt,
    ipc::NewChannelListener,
};
use std::sync::SyncRc;
use vidl::{internal::MemoryPermissions, CapabilityPtr, CapabilityRights};
use virtio::DeviceType;

enum InitEvent {
    Event(Event),
    Filesystem(Result<Vec<SyncRc<dyn Filesystem>>, DeviceError>),
}

#[derive(Debug)]
enum Event {
    Interrupt { interrupt: usize, block_device_index: usize },
    LoadingFinished(Result<(), Error>),
}

#[derive(Debug)]
enum Error {
    NoServicesFound,
}

#[present::main]
async fn main() {
    let fdt_ptr = std::env::a2() as *const u8;
    let fdt: fdt::Fdt<'static> = unsafe { fdt::Fdt::from_ptr(fdt_ptr).unwrap() };
    let fdt_size = fdt.total_size();

    let mut block_devices = Vec::new();
    let mut interrupts = Box::new(present::futures::stream::pending()) as Box<dyn Stream<Item = Event> + Unpin>;
    let mut join_handles = Vec::new();
    let mut interrupt_buffer = [0usize; 32];

    for node in fdt.all_nodes() {
        match node.compatible() {
            Some(compatible) => match compatible.all().any(|compat| compat == "virtio,mmio") {
                true => {}
                false => continue,
            },
            None => continue,
        }

        let cptr = librust::syscalls::io::claim_device(node.name).unwrap();
        let (mmio, n_interrupts) = query_mmio_cap(cptr, &mut interrupt_buffer).unwrap();

        let header = unsafe { &*mmio.address().cast::<virtio::VirtIoHeader>() };
        if header.device_type() != Some(DeviceType::BlockDevice) {
            // FIXME: release block device
            continue;
        }

        let virtio_device = filesystem::block_devices::virtio::VirtIoBlockDevice::new(unsafe {
            &*mmio.address().cast::<virtio::devices::block::VirtIoBlockDevice>()
        })
        .unwrap();

        let virtio_device = SyncRc::from_rc(std::rc::Rc::new(virtio_device) as std::rc::Rc<dyn BlockDevice>);
        let block_device_index = block_devices.len();

        for interrupt in &interrupt_buffer[..n_interrupts] {
            let interrupt = *interrupt;
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

    let (tx, rx) = present::sync::mpsc::unbounded();
    present::spawn(load_services(filesystems, unsafe { core::slice::from_raw_parts(fdt_ptr, fdt.total_size()) }, tx));

    let (interrupts, _) = core::pin::Pin::into_inner(init_stream).unmerge();
    let event_stream = interrupts.into_stream().merge(rx.into_stream().map(Event::LoadingFinished));
    present::pin!(event_stream);

    while let Some(event) = event_stream.next().await {
        match event {
            Event::Interrupt { interrupt, block_device_index } => {
                block_devices[block_device_index].handle_interrupt();
                librust::syscalls::io::complete_interrupt(interrupt).unwrap();
            }
            Event::LoadingFinished(res) => {
                println!("{res:?}");
                break;
            }
            _ => (),
        }
    }
}

async fn load_services(
    filesystems: SyncRc<[SyncRc<dyn Filesystem>]>,
    fdt: &'static [u8],
    tx: present::sync::mpsc::Sender<Result<(), Error>>,
) {
    let mut rootfs = None;
    let services_dir = Path::new("/services");

    for filesystem in &*filesystems {
        let root = filesystem.root();
        match filesystem.exists(Root::clone(&root), services_dir).await {
            Ok(Some(FileType::Directory)) => {
                rootfs = Some(filesystem);
                println!("Found services directory");
                break;
            }
            Ok(_) => continue,
            Err(e) => println!("Error searching filesystem for `/services` directory: {e:?}"),
        }
    }

    let rootfs = match rootfs {
        Some(fs) => fs,
        None => {
            tx.send(Err(Error::NoServicesFound));
            return;
        }
    };

    let root = rootfs.root();
    let files = rootfs.list_directory(Root::clone(&root), services_dir).await.unwrap();

    let stdio = &files[0];
    let devicemgr = &files[1];

    let devicemgr_file = rootfs
        .open(Root::clone(&root), &services_dir.join(Path::new(&devicemgr.filename)), FilePermissions::READ)
        .await
        .unwrap();
    let mut devicemgr_elf = Vec::new();
    while let Some((_, chunk)) = rootfs.read_file_block(FileId::clone(&devicemgr_file)).await.unwrap() {
        devicemgr_elf.extend_from_slice(&chunk);
    }

    let devicemgr = load_process("devicemgr", &devicemgr_elf, Some(fdt), &[]);

    let stdio_file = rootfs
        .open(Root::clone(&root), &services_dir.join(Path::new(&stdio.filename)), FilePermissions::READ)
        .await
        .unwrap();

    let mut stdio_elf = Vec::new();
    while let Some((_, chunk)) = rootfs.read_file_block(FileId::clone(&stdio_file)).await.unwrap() {
        stdio_elf.extend_from_slice(&chunk);
    }

    load_process("stdio", &stdio_elf, None, &[("devicemgr", devicemgr)]);

    tx.send(Ok(()));
}

fn load_process(name: &str, contents: &[u8], fdt: Option<&[u8]>, caps: &[(&str, CapabilityPtr)]) -> CapabilityPtr {
    let (mut space, mut env) = loadelf::load_elf(name, &loadelf::Elf::new(contents).unwrap()).unwrap();
    if let Some(fdt) = fdt {
        let mut fdt_obj = space.create_object(core::ptr::null(), fdt.len(), MemoryPermissions::READ).unwrap();
        fdt_obj.as_slice()[..fdt.len()].copy_from_slice(fdt);
        env.a2 = fdt_obj.vmspace_address() as usize;
    }
    for (name, cap) in caps {
        space.grant(name, *cap, CapabilityRights::READ | CapabilityRights::WRITE);
    }
    env.a0 = 0;
    env.a1 = 0;
    space.spawn(env).unwrap()
}
