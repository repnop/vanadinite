// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    drivers::{
        virtio::{
            mmio::block::{Command, CommandKind, VirtIoBlockDevice},
            queue::{DescriptorFlags, SplitVirtqueue},
            VirtIoDeviceError,
        },
        InterruptServicable,
    },
    mem::{
        paging::{PhysicalAddress, VirtualAddress},
        phys2virt, virt2phys,
    },
};
use alloc::{boxed::Box, collections::BTreeMap};

pub struct BlockDevice {
    device: &'static VirtIoBlockDevice,
    // TODO: allow for multiple queues
    queue: SplitVirtqueue,
    issued_commands: BTreeMap<usize, Box<Command>>,
}

impl BlockDevice {
    pub fn new(device: &'static VirtIoBlockDevice) -> Result<Self, VirtIoDeviceError> {
        let queue = SplitVirtqueue::new(64);

        device.init(&queue, 0)?;

        Ok(Self { device, queue, issued_commands: BTreeMap::new() })
    }

    pub fn queue_read(&mut self, sector: u64, read_to: PhysicalAddress) {
        let request = Box::into_raw(Box::new(Command { kind: CommandKind::Read, _reserved: 0, sector, status: 0 }));

        let desc1 = self.queue.alloc_descriptor().unwrap();
        let desc2 = self.queue.alloc_descriptor().unwrap();
        let desc3 = self.queue.alloc_descriptor().unwrap();

        let entry1 = &mut self.queue.descriptors[desc1];
        entry1.address = virt2phys(VirtualAddress::from_ptr(request));
        entry1.length = 16;
        entry1.flags = DescriptorFlags::Next;
        entry1.next = desc2 as u16;

        let entry2 = &mut self.queue.descriptors[desc2];
        entry2.address = read_to;
        entry2.length = 512;
        entry2.flags = DescriptorFlags::Next | DescriptorFlags::Write;
        entry2.next = desc3 as u16;

        let entry3 = &mut self.queue.descriptors[desc3];
        entry3.address = virt2phys(VirtualAddress::from_ptr(request).offset(16));
        entry3.length = 1;
        entry3.flags = DescriptorFlags::Write;

        let avail = &mut self.queue.available;
        avail.ring[avail.index as usize] = desc1 as u16;

        // FIXME: check for queue size overflow
        avail.index += 1;

        self.issued_commands.insert(desc1, unsafe { Box::from_raw(request) });

        self.device.header.queue_notify.notify();
    }
}

unsafe impl Send for BlockDevice {}
unsafe impl Sync for BlockDevice {}

impl InterruptServicable for BlockDevice {
    fn isr(_: usize, _: usize) -> Result<(), &'static str> {
        let mut this = crate::BLOCK_DEV.lock();
        let this = this.as_mut().unwrap();
        this.device.header.interrupt_ack.acknowledge_buffer_used();

        let desc1 = this.queue.used.ring[this.queue.used.index as usize].start_index as usize;
        let desc2 = this.queue.descriptors[desc1].next as usize;
        let desc3 = this.queue.descriptors[desc2].next as usize;

        let cmd: Box<Command> = this.issued_commands.remove(&desc1).unwrap();

        assert_eq!(cmd.status, 0);

        log::debug!("Successfully processed block device command");

        this.queue.free_descriptor(desc1);
        this.queue.free_descriptor(desc2);
        this.queue.free_descriptor(desc3);

        Ok(())
    }
}
