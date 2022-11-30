// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::BoxedFuture;

use super::{BlockDevice, DataBlock, DeviceError, SectorIndex};
use librust::mem::{DmaElement, DmaRegion, PhysicalAddress};
use present::sync::oneshot::{self, OneshotTx};
use std::collections::BTreeMap;
use std::sync::SyncRefCell;
use virtio::devices::block::{Command, CommandKind, CommandStatus};
use virtio::{
    splitqueue::{DescriptorFlags, SplitVirtqueue, SplitqueueIndex, VirtqueueDescriptor},
    StatusFlag, VirtIoDeviceError,
};

struct QueuedCommand {
    command_index: usize,
    data_index: usize,
    kind: QueuedCommandKind,
}

enum QueuedCommandKind {
    // FIXME: use a newtype or not need the command index at all
    Read(OneshotTx<Result<DataBlock, DeviceError>>),
    Write(OneshotTx<Result<(), DeviceError>>),
    Flush,
}

enum WaitingCommands {
    Read(SectorIndex, OneshotTx<Result<DataBlock, DeviceError>>),
    Write(SectorIndex, DataBlock, OneshotTx<Result<(), DeviceError>>),
    DataBlock(OneshotTx<DataBlock>),
}

struct VirtIoBlockDeviceInner {
    device: &'static virtio::devices::block::VirtIoBlockDevice,
    // TODO: allow for multiple queues
    queue: SplitVirtqueue,
    command_buffer: CommandBuffer,
    data_buffer: DataBuffer,
    queued_commands: BTreeMap<SplitqueueIndex<VirtqueueDescriptor>, QueuedCommand>,
    waiting_requests: VecDeque<WaitingCommands>,
}

pub struct VirtIoBlockDevice {
    inner: SyncRefCell<VirtIoBlockDeviceInner>,
}

impl VirtIoBlockDevice {
    pub fn new(device: &'static virtio::devices::block::VirtIoBlockDevice) -> Result<Self, VirtIoDeviceError> {
        Ok(Self {
            inner: SyncRefCell::new({
                let queue = SplitVirtqueue::new(64).unwrap();
                let command_buffer = CommandBuffer::new(512);
                let data_buffer = DataBuffer::new(512);

                device.header.status.reset();

                device.header.status.set_flag(StatusFlag::Acknowledge);
                device.header.status.set_flag(StatusFlag::Driver);

                // TODO: maybe use feature bits at some point
                let _ = device.header.features();

                device.header.driver_features_select.write(0);
                device.header.device_features_select.write(0);

                device.header.driver_features.write(0);

                device.header.status.set_flag(StatusFlag::FeaturesOk);

                if !device.header.status.is_set(StatusFlag::FeaturesOk) {
                    return Err(VirtIoDeviceError::FeaturesNotRecognized);
                }

                device.header.queue_select.write(0);
                device.header.queue_size.write(queue.queue_size());
                device.header.queue_descriptor.set(queue.descriptors.physical_address());
                device.header.queue_available.set(queue.available.physical_address());
                device.header.queue_used.set(queue.used.physical_address());

                device.header.queue_ready.ready();

                device.header.status.set_flag(StatusFlag::DriverOk);

                if device.header.status.failed() {
                    return Err(VirtIoDeviceError::DeviceError);
                }

                VirtIoBlockDeviceInner {
                    device,
                    queue,
                    command_buffer,
                    data_buffer,
                    queued_commands: BTreeMap::new(),
                    waiting_requests: VecDeque::new(),
                }
            }),
        })
    }
}

impl BlockDevice for VirtIoBlockDevice {
    fn block_size(&self) -> units::data::Bytes {
        // self.inner.borrow().device.block_size()
        // FIXME: use `device.block_size()`
        units::data::Bytes::new(512)
    }

    fn handle_interrupt(&self) {
        let mut this = self.inner.borrow_mut();
        let VirtIoBlockDeviceInner { queue, command_buffer, data_buffer, device, queued_commands, .. } = &mut *this;

        while let Some(used) = queue.used.pop() {
            let desc1 = SplitqueueIndex::new(used.start_index as u16);
            let desc2 = queue.descriptors.read(desc1).next;
            let desc3 = queue.descriptors.read(desc2).next;

            librust::mem::fence(librust::mem::FenceMode::Full);
            device.header.interrupt_ack.acknowledge_buffer_used();

            let QueuedCommand { command_index, data_index, kind } = queued_commands.remove(&desc1).unwrap();

            let command = command_buffer.get(command_index).unwrap();
            let data_block = data_buffer.get(data_index).unwrap();

            let command = command.get();
            let command_status = CommandStatus::from_u8(unsafe { (*command).status }).unwrap().into_result();

            if let Err(e) = command_status {
                println!("[filesystem] Disk error: {e:?}");
            }

            match kind {
                QueuedCommandKind::Flush => {}
                QueuedCommandKind::Read(tx) => match command_status {
                    Ok(_) => tx.send(unsafe { Ok(DataBlock::new(data_index, data_block.get(), |_, _| {})) }),
                    Err(_) => tx.send(Err(DeviceError::ReadError)),
                },
                QueuedCommandKind::Write(tx) => {
                    match command_status {
                        Ok(_) => tx.send(Ok(())),
                        Err(_) => tx.send(Err(DeviceError::ReadError)),
                    }

                    data_buffer.dealloc(data_index);
                }
            }

            queue.free_descriptor(desc1);
            queue.free_descriptor(desc2);
            queue.free_descriptor(desc3);
            command_buffer.dealloc(command_index);
        }

        // TODO: check for waiting commands
    }

    fn read_only(&self) -> bool {
        todo!()
    }

    fn alloc_data_block(&self) -> BoxedFuture<'static, DataBlock> {
        let mut this = self.inner.borrow_mut();
        match this.data_buffer.alloc() {
            Some((index, data_block)) => {
                Box::pin(core::future::ready(unsafe { DataBlock::new(index, data_block.get(), |_, _| {}) }))
            }
            None => {
                let (tx, rx) = oneshot::oneshot();
                this.waiting_requests.push_back(WaitingCommands::DataBlock(tx));
                Box::pin(async move { rx.recv().await })
            }
        }
    }

    fn flush(&self, range: core::ops::Range<SectorIndex>) -> BoxedFuture<'static, ()> {
        todo!()
    }

    fn read(&self, sector: SectorIndex) -> BoxedFuture<'static, Result<DataBlock, DeviceError>> {
        let mut this = self.inner.borrow_mut();

        let Some((desc1, desc2, desc3)) = this.get_descriptor_chain() else {
            let (tx, rx) = oneshot::oneshot();
            this.waiting_requests.push_back(WaitingCommands::Read(sector, tx));

            return Box::pin(async move {
                rx.recv().await
            });
        };

        let VirtIoBlockDeviceInner { device, queue, command_buffer, data_buffer, queued_commands, waiting_requests } =
            &mut *this;

        let Some((command_index, request)) = command_buffer.alloc() else {
            queue.free_descriptor(desc1);
            queue.free_descriptor(desc2);
            queue.free_descriptor(desc3);

            let (tx, rx) = oneshot::oneshot();
            waiting_requests.push_back(WaitingCommands::Read(sector, tx));

            return Box::pin(async move {
                rx.recv().await
            });
        };

        let Some((data_index, data_buffer)) = data_buffer.alloc() else {
            queue.free_descriptor(desc1);
            queue.free_descriptor(desc2);
            queue.free_descriptor(desc3);
            command_buffer.dealloc(command_index);

            let (tx, rx) = oneshot::oneshot();
            this.waiting_requests.push_back(WaitingCommands::Read(sector, tx));

            return Box::pin(async move {
                rx.recv().await
            });
        };

        let descriptor_flags = DescriptorFlags::NEXT | DescriptorFlags::WRITE;
        unsafe { *request.get() = Command { kind: CommandKind::Read, _reserved: 0, sector: sector.get(), status: 0 } };

        queue.descriptors.write(
            desc1,
            VirtqueueDescriptor {
                address: request.physical_address(),
                length: 16,
                flags: DescriptorFlags::NEXT,
                next: desc2,
            },
        );

        queue.descriptors.write(
            desc2,
            VirtqueueDescriptor {
                address: data_buffer.physical_address(),
                length: 512,
                flags: descriptor_flags,
                next: desc3,
            },
        );

        queue.descriptors.write(
            desc3,
            VirtqueueDescriptor {
                address: PhysicalAddress::new(request.physical_address().as_usize() + 16),
                length: 1,
                flags: DescriptorFlags::WRITE,
                ..Default::default()
            },
        );

        let (tx, rx) = oneshot::oneshot();

        queue.available.push(desc1);
        queued_commands.insert(desc1, QueuedCommand { command_index, data_index, kind: QueuedCommandKind::Read(tx) });

        // Fence the MMIO register write since its not guaranteed to be in the
        // same order relative to RAM read/writes
        librust::mem::fence(librust::mem::FenceMode::Write);
        device.header.queue_notify.notify(0);

        Box::pin(async move { rx.recv().await })
    }

    fn write(&self, sector: SectorIndex, block: DataBlock) -> BoxedFuture<'static, Result<(), DeviceError>> {
        let mut this = self.inner.borrow_mut();

        let Some((desc1, desc2, desc3)) = this.get_descriptor_chain() else {
            let (tx, rx) = oneshot::oneshot();
            this.waiting_requests.push_back(WaitingCommands::Write(sector, block, tx));

            return Box::pin(async move {
                rx.recv().await
            });
        };

        let VirtIoBlockDeviceInner { device, queue, command_buffer, data_buffer, queued_commands, waiting_requests } =
            &mut *this;

        let (data_index, ptr) = DataBlock::leak(block);
        let Some(data_buffer) = data_buffer.get(data_index) else {
            return Box::pin(core::future::ready(Err(DeviceError::WriteError)));
        };

        let Some((command_index, request)) = command_buffer.alloc() else {
            queue.free_descriptor(desc1);
            queue.free_descriptor(desc2);
            queue.free_descriptor(desc3);

            let (tx, rx) = oneshot::oneshot();
            waiting_requests.push_back(WaitingCommands::Write(sector, unsafe { DataBlock::new(data_index, ptr, |_, _| {})}, tx));

            return Box::pin(async move {
                rx.recv().await
            });
        };

        let descriptor_flags = DescriptorFlags::NEXT;
        unsafe { *request.get() = Command { kind: CommandKind::Write, _reserved: 0, sector: sector.get(), status: 0 } };

        queue.descriptors.write(
            desc1,
            VirtqueueDescriptor {
                address: request.physical_address(),
                length: 16,
                flags: DescriptorFlags::NEXT,
                next: desc2,
            },
        );

        queue.descriptors.write(
            desc2,
            VirtqueueDescriptor {
                address: data_buffer.physical_address(),
                length: 512,
                flags: descriptor_flags,
                next: desc3,
            },
        );

        queue.descriptors.write(
            desc3,
            VirtqueueDescriptor {
                address: PhysicalAddress::new(request.physical_address().as_usize() + 16),
                length: 1,
                flags: DescriptorFlags::WRITE,
                ..Default::default()
            },
        );

        let (tx, rx) = oneshot::oneshot();

        queue.available.push(desc1);
        queued_commands.insert(desc1, QueuedCommand { command_index, data_index, kind: QueuedCommandKind::Write(tx) });

        // Fence the MMIO register write since its not guaranteed to be in the
        // same order relative to RAM read/writes
        librust::mem::fence(librust::mem::FenceMode::Write);
        device.header.queue_notify.notify(0);

        Box::pin(async move { rx.recv().await })
    }
}

impl VirtIoBlockDeviceInner {
    fn get_descriptor_chain(
        &mut self,
    ) -> Option<(
        SplitqueueIndex<VirtqueueDescriptor>,
        SplitqueueIndex<VirtqueueDescriptor>,
        SplitqueueIndex<VirtqueueDescriptor>,
    )> {
        match self.queue.alloc_descriptor() {
            Some(first) => match self.queue.alloc_descriptor() {
                Some(second) => match self.queue.alloc_descriptor() {
                    Some(third) => Some((first, second, third)),
                    None => {
                        self.queue.free_descriptor(first);
                        self.queue.free_descriptor(second);
                        None
                    }
                },
                None => {
                    self.queue.free_descriptor(first);
                    None
                }
            },
            None => None,
        }
    }
}

// TODO: command and data buffers can probably be merged?

struct CommandBuffer {
    buffer: DmaRegion<[Command]>,
    free_indices: VecDeque<u16>,
}

impl CommandBuffer {
    fn new(len: usize) -> Self {
        Self {
            buffer: unsafe { DmaRegion::zeroed_many(len).unwrap().assume_init() },
            free_indices: (0..len as u16).collect(),
        }
    }

    fn alloc(&mut self) -> Option<(usize, DmaElement<'_, Command>)> {
        let index = self.free_indices.pop_front()? as usize;
        Some((index, self.buffer.get(index).unwrap()))
    }

    fn dealloc(&mut self, index: usize) {
        let index = u16::try_from(index).expect("invalid index supplied");
        assert!(!self.free_indices.contains(&index));
        self.free_indices.push_back(index);
    }

    fn get(&mut self, index: usize) -> Option<DmaElement<'_, Command>> {
        self.buffer.get(index)
    }
}

struct DataBuffer {
    buffer: DmaRegion<[[u8; 512]]>,
    free_indices: VecDeque<u16>,
}

impl DataBuffer {
    fn new(len: usize) -> Self {
        Self {
            buffer: unsafe { DmaRegion::zeroed_many(len).unwrap().assume_init() },
            free_indices: (0..len as u16).collect(),
        }
    }

    fn alloc(&mut self) -> Option<(usize, DmaElement<'_, [u8; 512]>)> {
        let index = self.free_indices.pop_front()? as usize;
        Some((index, self.buffer.get(index).unwrap()))
    }

    fn dealloc(&mut self, index: usize) {
        let index = u16::try_from(index).expect("invalid index supplied");
        assert!(!self.free_indices.contains(&index));
        self.free_indices.push_back(index);
    }

    fn get(&mut self, index: usize) -> Option<DmaElement<'_, [u8; 512]>> {
        self.buffer.get(index)
    }
}

// impl InterruptServicable for BlockDevice {
//     fn isr(_: usize, _: usize) -> Result<(), &'static str> {
//         let mut this = crate::BLOCK_DEV.lock();
//         let this = this.as_mut().unwrap();
//         this.device.header.interrupt_ack.acknowledge_buffer_used();
//
//         let desc1 = this.queue.used.ring[this.queue.used.index as usize].start_index as usize;
//         let desc2 = this.queue.descriptors[desc1].next as usize;
//         let desc3 = this.queue.descriptors[desc2].next as usize;
//
//         let cmd: Box<Command> = this.issued_commands.remove(&desc1).unwrap();
//
//         assert_eq!(cmd.status, 0);
//
//         log::debug!("Successfully processed block device command");
//
//         this.queue.free_descriptor(desc1);
//         this.queue.free_descriptor(desc2);
//         this.queue.free_descriptor(desc3);
//
//         Ok(())
//     }
// }
