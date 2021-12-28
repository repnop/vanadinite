// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod block_device;

pub use block_device::VirtIoBlockDevice;

use block_device::{Command, CommandKind};
use librust::mem::{DmaElement, DmaRegion, PhysicalAddress};
use std::collections::BTreeMap;
use virtio::{
    splitqueue::{DescriptorFlags, SplitVirtqueue},
    VirtIoDeviceError,
};

pub struct BlockDevice {
    device: &'static VirtIoBlockDevice,
    // TODO: allow for multiple queues
    queue: SplitVirtqueue,
    command_buffer: CommandBuffer,
    issued_commands: BTreeMap<usize, usize>,
}

impl BlockDevice {
    pub fn new(device: &'static VirtIoBlockDevice) -> Result<Self, VirtIoDeviceError> {
        let queue = SplitVirtqueue::new(64).unwrap();
        let command_buffer = CommandBuffer::new(512);

        device.init(&queue, 0)?;

        Ok(Self { device, queue, command_buffer, issued_commands: BTreeMap::new() })
    }

    pub fn queue_read(&mut self, sector: u64, read_to: PhysicalAddress) -> usize {
        let (index, mut request) = self.command_buffer.alloc().unwrap();

        unsafe { *request.get_mut() = Command { kind: CommandKind::Read, _reserved: 0, sector, status: 0 } };

        let desc1 = self.queue.alloc_descriptor().unwrap();
        let desc2 = self.queue.alloc_descriptor().unwrap();
        let desc3 = self.queue.alloc_descriptor().unwrap();

        let entry1 = &mut self.queue.descriptors[desc1];
        entry1.address = request.physical_address();
        entry1.length = 16;
        entry1.flags = DescriptorFlags::NEXT;
        entry1.next = desc2 as u16;

        let entry2 = &mut self.queue.descriptors[desc2];
        entry2.address = read_to;
        entry2.length = 512;
        entry2.flags = DescriptorFlags::NEXT | DescriptorFlags::WRITE;
        entry2.next = desc3 as u16;

        let entry3 = &mut self.queue.descriptors[desc3];
        entry3.address = PhysicalAddress::new(request.physical_address().as_usize() + 16);
        entry3.length = 1;
        entry3.flags = DescriptorFlags::WRITE;

        let avail = &mut self.queue.available;
        let avail_index = avail.index as usize;
        avail.ring[avail_index] = desc1 as u16;

        // FIXME: check for queue size overflow
        avail.index += 1;

        self.issued_commands.insert(desc1, index);

        // Fence the MMIO register write since its not guaranteed to be in the
        // same order relative to RAM read/writes
        librust::mem::fence(librust::mem::FenceMode::Full);

        self.device.header.queue_notify.notify();

        desc1
    }
}

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
