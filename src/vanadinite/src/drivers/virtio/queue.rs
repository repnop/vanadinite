// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ptr::slice_from_raw_parts_mut;

use crate::mem::paging::{PhysicalAddress, VirtualAddress};
use alloc::{alloc::alloc_zeroed, alloc::Layout, boxed::Box, vec, vec::Vec};

pub struct SplitVirtqueue {
    queue_size: usize,
    free_bitmap: Vec<u64>,
    pub descriptors: Vec<VirtqueueDescriptor>,
    // TODO: figure out if its valid to use Box here, the underlying device will
    // be writing to the memory so there may be optimization concerns with
    // reads/writes
    pub available: Box<VirtqueueAvailable>,
    pub used: Box<VirtqueueUsed>,
}

impl SplitVirtqueue {
    pub fn new(queue_size: usize) -> Self {
        assert!(queue_size.is_power_of_two(), "non-power of two size queue");
        assert!(queue_size <= 32768, "max queue size exceeded");

        let free_bitmap = vec![0; queue_size / 64 + 1];

        let avail_size = 6 + 2 * queue_size;
        let used_size = 6 + 8 * queue_size;

        // FIXME: this should be align 16, but allocator doesn't support that atm
        let descriptors = vec![
            VirtqueueDescriptor {
                address: PhysicalAddress::new(0),
                flags: DescriptorFlags::None,
                next: 0,
                length: 0,
            };
            queue_size
        ];

        // Safety:
        // https://users.rust-lang.org/t/construct-fat-pointer-to-struct/29198/9
        // I'm not sure if the `*mut [()]` is necessary, but just in case
        let available = unsafe {
            Box::from_raw(slice_from_raw_parts_mut(
                alloc_zeroed(Layout::from_size_align(avail_size, 2).unwrap()) as *mut (),
                queue_size,
            ) as *mut VirtqueueAvailable)
        };

        let used = unsafe {
            Box::from_raw(slice_from_raw_parts_mut(
                alloc_zeroed(Layout::from_size_align(used_size, 2).unwrap()) as *mut (),
                queue_size,
            ) as *mut VirtqueueUsed)
        };

        let mut this = Self { queue_size, free_bitmap, descriptors, available, used };

        this.available.index = 0;
        this.used.index = 0;

        for i in 0..queue_size {
            this.descriptors[i].next = i as u16 + 1;
        }

        this
    }

    pub fn alloc_descriptor(&mut self) -> Option<usize> {
        let (index, entry) = self.free_bitmap.iter_mut().enumerate().find(|(_, e)| **e != u64::max_value())?;
        let bit_index = entry.trailing_ones() as usize;

        let descriptor_index = index * 64 + bit_index;

        if descriptor_index > self.queue_size {
            return None;
        }

        *entry |= 1 << bit_index;

        Some(descriptor_index)
    }

    pub fn free_descriptor(&mut self, index: usize) {
        let (index, bit_index) = (index / 64, index % 64);
        let entry = &mut self.free_bitmap[index];

        assert_eq!((*entry >> bit_index) & 1, 1, "double-freeing descriptor");

        *entry &= !(1 << bit_index);
    }

    pub fn queue_size(&self) -> u32 {
        self.queue_size as u32
    }

    pub fn descriptor_physical_address(&self) -> PhysicalAddress {
        crate::mem::virt2phys(VirtualAddress::from_ptr(self.descriptors.as_ptr()))
    }

    pub fn available_physical_address(&self) -> PhysicalAddress {
        crate::mem::virt2phys(VirtualAddress::from_ptr(&*self.available as *const _ as *const u8))
    }

    pub fn used_physical_address(&self) -> PhysicalAddress {
        crate::mem::virt2phys(VirtualAddress::from_ptr(&*self.used as *const _ as *const u8))
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtqueueDescriptor {
    pub address: PhysicalAddress,
    pub length: u32,
    pub flags: DescriptorFlags,
    pub next: u16,
}

#[repr(C)]
pub struct VirtqueueAvailable {
    pub flags: u16,
    pub index: u16,
    pub ring: [u16],
}

pub struct VirtqueueUsed {
    pub flags: u16,
    pub index: u16,
    pub ring: [VirtqueueUsedElement],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtqueueUsedElement {
    pub start_index: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct DescriptorFlags(u16);

#[allow(non_upper_case_globals)]
impl DescriptorFlags {
    pub const None: Self = Self(0);
    pub const Next: Self = Self(1);
    pub const Write: Self = Self(2);
    pub const Indirect: Self = Self(4);
}

impl core::ops::BitOr for DescriptorFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
