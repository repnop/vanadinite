// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::mem::{DmaRegion, PhysicalAddress};

pub struct SplitVirtqueue {
    queue_size: usize,
    freelist: VecDeque<u16>,
    pub descriptors: DmaRegion<[VirtqueueDescriptor]>,
    pub available: DmaRegion<VirtqueueAvailable>,
    pub used: DmaRegion<VirtqueueUsed>,
}

impl SplitVirtqueue {
    pub fn new(queue_size: usize) -> Result<Self, SplitVirtqueueError> {
        match queue_size {
            n if !n.is_power_of_two() => return Err(SplitVirtqueueError::NotPowerOfTwo),
            0..=32768 => {}
            _ => return Err(SplitVirtqueueError::TooLarge),
        }

        let freelist = (0..queue_size as u16).collect();

        // FIXME: return errors
        let descriptors = unsafe { DmaRegion::zeroed(queue_size).unwrap().assume_init() };
        let available = unsafe { DmaRegion::new_raw(queue_size, true).unwrap() };
        let used = unsafe { DmaRegion::new_raw(queue_size, true).unwrap() };

        let mut this = Self { queue_size, freelist, descriptors, available, used };

        this.available.index = 0;
        this.used.index = 0;

        for i in 0..queue_size {
            this.descriptors[i].next = i as u16 + 1;
        }

        Ok(this)
    }

    pub fn alloc_descriptor(&mut self) -> Option<usize> {
        self.freelist.pop_front().map(|n| n as usize)
    }

    pub fn free_descriptor(&mut self, index: usize) {
        self.freelist.push_back(index as u16)
    }

    pub fn queue_size(&self) -> u32 {
        self.queue_size as u32
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SplitVirtqueueError {
    MemoryAllocationError,
    NotPowerOfTwo,
    TooLarge,
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

#[repr(C)]
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

impl DescriptorFlags {
    pub const NONE: Self = Self(0);
    pub const NEXT: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const INDIRECT: Self = Self(4);
}

impl core::ops::BitOr for DescriptorFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
