// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::mem::{DmaElement, DmaRegion, PhysicalAddress};
use std::collections::BTreeMap;
use virtio::devices::block::{Command, CommandError, CommandKind, CommandStatus};
use virtio::{
    devices::block::VirtIoBlockDevice,
    splitqueue::{DescriptorFlags, SplitVirtqueue, SplitqueueIndex, VirtqueueDescriptor},
    StatusFlag, VirtIoDeviceError,
};

#[derive(Debug, Clone, Copy)]
pub enum OperationRequest<'a> {
    Read { sector: u64 },
    Write { sector: u64, data: &'a [u8] },
}

#[derive(Debug, Clone, Copy)]
pub enum OperationResult {
    Read([u8; 512]),
    Write,
}

#[derive(Debug, Clone, Copy)]
pub enum Error {
    CommandError(CommandError),
    NoCommandCompletion,
}

impl From<CommandError> for Error {
    fn from(e: CommandError) -> Self {
        Self::CommandError(e)
    }
}

pub struct BlockDevice {
    device: &'static VirtIoBlockDevice,
    // TODO: allow for multiple queues
    queue: SplitVirtqueue,
    command_buffer: CommandBuffer,
    data_buffer: DataBuffer,
    issued_commands: BTreeMap<SplitqueueIndex<VirtqueueDescriptor>, (usize, usize)>,
}

impl BlockDevice {
    pub fn new(device: &'static VirtIoBlockDevice) -> Result<Self, VirtIoDeviceError> {
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

        Ok(Self { device, queue, command_buffer, data_buffer, issued_commands: BTreeMap::new() })
    }

    fn queue_command(&mut self, operation: OperationRequest<'_>) {
        let (command_index, mut request) = self.command_buffer.alloc().unwrap();
        let (data_index, mut buffer) = self.data_buffer.alloc().unwrap();
        let (sector, descriptor_flag, length) = match operation {
            OperationRequest::Read { sector } => (sector, DescriptorFlags::NEXT | DescriptorFlags::WRITE, 512),
            OperationRequest::Write { sector, data } => (sector, DescriptorFlags::NEXT, data.len().min(512)),
        };

        *request.get_mut() = Command {
            kind: match operation {
                OperationRequest::Read { .. } => CommandKind::Read,
                OperationRequest::Write { .. } => CommandKind::Write,
            },
            _reserved: 0,
            sector,
            status: 0,
        };

        if let OperationRequest::Write { data, .. } = operation {
            buffer.get_mut()[..length].copy_from_slice(&data[..length]);
        }

        let desc1 = self.queue.alloc_descriptor().unwrap();
        let desc2 = self.queue.alloc_descriptor().unwrap();
        let desc3 = self.queue.alloc_descriptor().unwrap();

        self.queue.descriptors.write(
            desc1,
            VirtqueueDescriptor {
                address: request.physical_address(),
                length: 16,
                flags: DescriptorFlags::NEXT,
                next: desc2,
            },
        );

        self.queue.descriptors.write(
            desc2,
            VirtqueueDescriptor {
                address: buffer.physical_address(),
                length: length as u32,
                flags: descriptor_flag,
                next: desc3,
            },
        );

        self.queue.descriptors.write(
            desc3,
            VirtqueueDescriptor {
                address: PhysicalAddress::new(request.physical_address().as_usize() + 16),
                length: 1,
                flags: DescriptorFlags::WRITE,
                ..Default::default()
            },
        );

        self.queue.available.push(desc1);

        self.issued_commands.insert(desc1, (command_index, data_index));

        // Fence the MMIO register write since its not guaranteed to be in the
        // same order relative to RAM read/writes
        librust::mem::fence(librust::mem::FenceMode::Write);

        self.device.header.queue_notify.notify(0);
    }

    pub fn queue_read(&mut self, sector: u64) {
        self.queue_command(OperationRequest::Read { sector });
    }

    pub fn queue_write(&mut self, sector: u64, data: &[u8]) {
        self.queue_command(OperationRequest::Write { sector, data });
    }

    pub fn finish_command(&mut self) -> Result<OperationResult, Error> {
        let desc1 = SplitqueueIndex::new(self.queue.used.pop().ok_or(Error::NoCommandCompletion)?.start_index as u16);
        let desc2 = self.queue.descriptors.read(desc1).next;
        let desc3 = self.queue.descriptors.read(desc2).next;

        librust::mem::fence(librust::mem::FenceMode::Full);
        self.device.header.interrupt_ack.acknowledge_buffer_used();

        let (command_idx, data_idx) = self.issued_commands.remove(&desc1).unwrap();
        let command = self.command_buffer.get(command_idx).unwrap();
        let data = self.data_buffer.get(data_idx).unwrap();

        self.queue.free_descriptor(desc1);
        self.queue.free_descriptor(desc2);
        self.queue.free_descriptor(desc3);

        let command = command.get();
        CommandStatus::from_u8(command.status).unwrap().into_result()?;

        let ret = match command.kind {
            CommandKind::Read => Ok(OperationResult::Read(*data.get())),
            CommandKind::Write => Ok(OperationResult::Write),
            _ => todo!(),
        };

        self.command_buffer.dealloc(command_idx);
        self.data_buffer.dealloc(data_idx);

        ret
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
