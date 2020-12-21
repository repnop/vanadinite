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
use alloc::boxed::Box;

pub struct BlockDevice {
    device: &'static VirtIoBlockDevice,
    // TODO: allow for multiple queues
    queue: SplitVirtqueue,
}

impl BlockDevice {
    pub fn new(device: &'static VirtIoBlockDevice) -> Result<Self, VirtIoDeviceError> {
        let queue = SplitVirtqueue::new(64);

        device.init(&queue, 0)?;

        Ok(Self { device, queue })
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

        let desc1 = this.queue.used.ring[0].start_index as usize;
        let desc2 = this.queue.descriptors[desc1].next as usize;
        let desc3 = this.queue.descriptors[desc2].next as usize;

        let cmd_addr = phys2virt(this.queue.descriptors[desc1].address).as_mut_ptr().cast();
        let cmd: Box<Command> = unsafe { Box::from_raw(cmd_addr) };

        assert_eq!(cmd.status, 0);

        let schweet_data = phys2virt(this.queue.descriptors[desc2].address);
        let schweet_data = unsafe { core::slice::from_raw_parts(schweet_data.as_ptr(), 512) };
        log::info!(
            "yay we read something @ {:#p}: {}",
            phys2virt(this.queue.descriptors[desc2].address),
            alloc::string::String::from_utf8_lossy(schweet_data),
        );

        this.queue.free_descriptor(desc1);
        this.queue.free_descriptor(desc2);
        this.queue.free_descriptor(desc3);

        Ok(())
    }
}
