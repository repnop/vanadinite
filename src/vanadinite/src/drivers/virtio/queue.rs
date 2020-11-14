// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::{
        paging::{PhysicalAddress, VirtualAddress},
        phys::PhysicalPage,
    },
    utils::volatile::{Read, ReadWrite, Volatile},
    PhysicalMemoryAllocator,
};

pub struct SplitVirtqueue {
    queue_size: usize,
    phys_start: PhysicalPage,
    descriptors: *mut u8,
    available: *mut u8,
    used: *mut u8,
}

impl SplitVirtqueue {
    pub fn alloc(queue_size: usize) -> Self {
        assert!(queue_size.is_power_of_two(), "non-power of two size queue");
        assert!(queue_size <= 32768, "max queue size exceeded");

        let desc_size = 16 * queue_size;
        let desc_off_to_avail = (desc_size as *const u8).align_offset(2);
        let avail_size = 6 + 2 * queue_size;
        let avail_off_to_used = (avail_size as *const u8).align_offset(4);
        let used_size = 6 + 8 * queue_size;

        let total_size = desc_size + desc_off_to_avail + avail_size + avail_off_to_used + used_size;

        let pages_needed = (total_size / 4096) + (total_size % 4096 != 0) as usize;

        let phys_start = unsafe { crate::PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(pages_needed).unwrap() };
        let virt_start = crate::kernel_patching::phys2virt(phys_start.as_phys_address()).as_mut_ptr();
        let descriptors = virt_start.cast();
        let available = unsafe { virt_start.add(desc_size + desc_off_to_avail).cast() };
        let used = unsafe { virt_start.add(desc_size + desc_off_to_avail + avail_size + avail_off_to_used).cast() };

        let mut this = Self { queue_size, phys_start, descriptors, available, used };

        this.available_mut().index().write(0);
        this.used_mut().index().write(0);
        for i in 0..queue_size as u16 {
            this.descriptor_at_mut(i as usize).next().write(i + 1);
        }

        this
    }

    pub fn queue_size(&self) -> u32 {
        self.queue_size as u32
    }

    pub fn descriptor_physical_address(&self) -> PhysicalAddress {
        crate::kernel_patching::virt2phys(VirtualAddress::from_ptr(self.descriptors))
    }

    pub fn descriptor_at(&self, index: usize) -> VirtqueueDescriptor<'_> {
        assert!(index < self.queue_size, "attempted to index outside of current queue size");
        unsafe {
            VirtqueueDescriptor {
                address_lo: self.descriptors.add(16 * self.queue_size).cast(),
                address_hi: self.descriptors.add(16 * self.queue_size + 4).cast(),
                length: self.descriptors.add(16 * self.queue_size + 8).cast(),
                flags: self.descriptors.add(16 * self.queue_size + 12).cast(),
                next: self.descriptors.add(16 * self.queue_size + 14).cast(),
                _l: core::marker::PhantomData,
            }
        }
    }

    pub fn descriptor_at_mut(&mut self, index: usize) -> VirtqueueDescriptorMut<'_> {
        assert!(index < self.queue_size, "attempted to index outside of current queue size");
        unsafe {
            VirtqueueDescriptorMut {
                address_lo: self.descriptors.add(16 * self.queue_size).cast(),
                address_hi: self.descriptors.add(16 * self.queue_size + 4).cast(),
                length: self.descriptors.add(16 * self.queue_size + 8).cast(),
                flags: self.descriptors.add(16 * self.queue_size + 12).cast(),
                next: self.descriptors.add(16 * self.queue_size + 14).cast(),
                _l: core::marker::PhantomData,
            }
        }
    }

    pub fn available_physical_address(&self) -> PhysicalAddress {
        crate::kernel_patching::virt2phys(VirtualAddress::from_ptr(self.available))
    }

    pub fn available(&self) -> VirtqueueAvailable<'_> {
        unsafe {
            VirtqueueAvailable {
                flags: self.available.cast(),
                index: self.available.add(2).cast(),
                ring: self.available.add(4).cast(),
                ring_size: self.queue_size,
                _l: core::marker::PhantomData,
            }
        }
    }

    pub fn available_mut(&self) -> VirtqueueAvailableMut<'_> {
        unsafe {
            VirtqueueAvailableMut {
                flags: self.available.cast(),
                index: self.available.add(2).cast(),
                ring: self.available.add(4).cast(),
                ring_size: self.queue_size,
                _l: core::marker::PhantomData,
            }
        }
    }

    pub fn used_physical_address(&self) -> PhysicalAddress {
        crate::kernel_patching::virt2phys(VirtualAddress::from_ptr(self.used))
    }

    pub fn used(&self) -> VirtqueueAvailable<'_> {
        unsafe {
            VirtqueueAvailable {
                flags: self.used.cast(),
                index: self.used.add(2).cast(),
                ring: self.used.add(4).cast(),
                ring_size: self.queue_size,
                _l: core::marker::PhantomData,
            }
        }
    }

    pub fn used_mut(&self) -> VirtqueueAvailableMut<'_> {
        unsafe {
            VirtqueueAvailableMut {
                flags: self.used.cast(),
                index: self.used.add(2).cast(),
                ring: self.used.add(4).cast(),
                ring_size: self.queue_size,
                _l: core::marker::PhantomData,
            }
        }
    }
}

#[repr(C)]
pub struct VirtqueueDescriptor<'a> {
    address_lo: *const u32,
    address_hi: *const u32,
    length: *const u32,
    flags: *const u16,
    next: *const u16,
    _l: core::marker::PhantomData<&'a ()>,
}

impl VirtqueueDescriptor<'_> {
    pub fn address(&self) -> PhysicalAddress {
        let address_lo = unsafe { self.address_lo.read_volatile() } as usize;
        let address_hi = unsafe { self.address_hi.read_volatile() } as usize;

        PhysicalAddress::new((address_hi << 32) | address_lo)
    }

    pub fn length(&self) -> &Volatile<u32, Read> {
        unsafe { &*self.length.cast() }
    }

    pub fn flags(&self) -> &Volatile<u16, Read> {
        unsafe { &*self.flags.cast() }
    }

    pub fn next(&self) -> &Volatile<u16, Read> {
        unsafe { &*self.flags.cast() }
    }
}

#[repr(C)]
pub struct VirtqueueDescriptorMut<'a> {
    address_lo: *mut u32,
    address_hi: *mut u32,
    length: *mut u32,
    flags: *mut u16,
    next: *mut u16,
    _l: core::marker::PhantomData<&'a mut ()>,
}

impl VirtqueueDescriptorMut<'_> {
    pub fn address(&self, addr: PhysicalAddress) {
        unsafe {
            self.address_lo.write_volatile(addr.as_usize() as u32);
            self.address_hi.write_volatile((addr.as_usize() >> 32) as u32);
        }
    }

    pub fn length(&mut self) -> &mut Volatile<u32, ReadWrite> {
        unsafe { &mut *self.length.cast() }
    }

    pub fn flags(&mut self) -> &mut Volatile<u16, ReadWrite> {
        unsafe { &mut *self.flags.cast() }
    }

    pub fn next(&mut self) -> &mut Volatile<u16, ReadWrite> {
        unsafe { &mut *self.flags.cast() }
    }
}

pub struct VirtqueueAvailable<'a> {
    flags: *const u16,
    index: *const u16,
    ring: *const u16,
    ring_size: usize,
    _l: core::marker::PhantomData<&'a ()>,
}

impl VirtqueueAvailable<'_> {
    pub fn flags(&self) -> &Volatile<u16, Read> {
        unsafe { &*self.flags.cast() }
    }

    pub fn index(&self) -> &Volatile<u16, Read> {
        unsafe { &*self.index.cast() }
    }

    pub fn ring(&self) -> &[Volatile<u16, Read>] {
        unsafe { core::slice::from_raw_parts(self.ring.cast(), self.ring_size) }
    }
}

pub struct VirtqueueAvailableMut<'a> {
    flags: *mut u16,
    index: *mut u16,
    ring: *mut u16,
    ring_size: usize,
    _l: core::marker::PhantomData<&'a mut ()>,
}

impl VirtqueueAvailableMut<'_> {
    pub fn flags(&mut self) -> &mut Volatile<u16, ReadWrite> {
        unsafe { &mut *self.flags.cast() }
    }

    pub fn index(&mut self) -> &mut Volatile<u16, ReadWrite> {
        unsafe { &mut *self.index.cast() }
    }

    pub fn ring(&mut self) -> &mut [Volatile<u16, ReadWrite>] {
        unsafe { core::slice::from_raw_parts_mut(self.ring.cast(), self.ring_size) }
    }
}

pub struct VirtqueueUsed<'a> {
    flags: *const u16,
    index: *const u16,
    ring: *const u16,
    ring_size: usize,
    _l: core::marker::PhantomData<&'a ()>,
}

impl VirtqueueUsed<'_> {
    pub fn flags(&self) -> &Volatile<u16, Read> {
        unsafe { &*self.flags.cast() }
    }

    pub fn index(&self) -> &Volatile<u16, Read> {
        unsafe { &*self.index.cast() }
    }

    pub fn ring(&self) -> &[Volatile<VirtqueueUsedElement, Read>] {
        unsafe { core::slice::from_raw_parts(self.ring.cast(), self.ring_size) }
    }
}

pub struct VirtqueueUsedMut<'a> {
    flags: *mut u16,
    index: *mut u16,
    ring: *mut u16,
    ring_size: usize,
    _l: core::marker::PhantomData<&'a mut ()>,
}

impl VirtqueueUsedMut<'_> {
    pub fn flags(&mut self) -> &mut Volatile<u16, ReadWrite> {
        unsafe { &mut *self.flags.cast() }
    }

    pub fn index(&mut self) -> &mut Volatile<u16, ReadWrite> {
        unsafe { &mut *self.index.cast() }
    }

    pub fn ring(&mut self) -> &mut [Volatile<VirtqueueUsedElement, ReadWrite>] {
        unsafe { core::slice::from_raw_parts_mut(self.ring.cast(), self.ring_size) }
    }
}

#[repr(C)]
pub struct VirtqueueUsedElement {
    pub start_index: u32,
    pub length: u32,
}
