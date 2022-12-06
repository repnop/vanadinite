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
    pub descriptors: DescriptorQueue,
    pub available: AvailableQueue,
    pub used: UsedQueue,
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
        let descriptors =
            DescriptorQueue { queue: unsafe { DmaRegion::zeroed_many(queue_size).unwrap().assume_init() } };
        let available = AvailableQueue { queue: unsafe { DmaRegion::new_raw(queue_size, true).unwrap() } };
        let used = UsedQueue { queue: unsafe { DmaRegion::new_raw(queue_size, true).unwrap() }, last_seen: 0 };

        Ok(Self { queue_size, freelist, descriptors, available, used })
    }

    pub fn alloc_descriptor(&mut self) -> Option<SplitqueueIndex<VirtqueueDescriptor>> {
        self.freelist.pop_back().map(SplitqueueIndex::new)
    }

    pub fn free_descriptor(&mut self, index: SplitqueueIndex<VirtqueueDescriptor>) {
        self.freelist.push_back(index.0)
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

#[repr(transparent)]
pub struct SplitqueueIndex<T>(u16, core::marker::PhantomData<T>);

impl<T> core::fmt::Debug for SplitqueueIndex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SplitqueueIndex").field("0", &self.0).finish()
    }
}

impl<T> Copy for SplitqueueIndex<T> {}
impl<T> Clone for SplitqueueIndex<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> core::cmp::Eq for SplitqueueIndex<T> {}
impl<T> core::cmp::PartialEq for SplitqueueIndex<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> core::hash::Hash for SplitqueueIndex<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u16(self.0);
    }
}

impl<T> core::cmp::PartialOrd for SplitqueueIndex<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T> core::cmp::Ord for SplitqueueIndex<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> SplitqueueIndex<T> {
    pub fn new(index: u16) -> Self {
        Self(index, core::marker::PhantomData)
    }
}

pub struct DescriptorQueue {
    queue: DmaRegion<[VirtqueueDescriptor]>,
}

impl DescriptorQueue {
    pub fn physical_address(&self) -> PhysicalAddress {
        self.queue.physical_address()
    }

    pub fn write(&mut self, index: SplitqueueIndex<VirtqueueDescriptor>, descriptor: VirtqueueDescriptor) {
        unsafe { core::ptr::write_volatile(&mut self.queue[index.0 as usize], descriptor) };
    }

    pub fn read(&self, index: SplitqueueIndex<VirtqueueDescriptor>) -> VirtqueueDescriptor {
        unsafe { core::ptr::read_volatile(&self.queue[index.0 as usize]) }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtqueueDescriptor {
    pub address: PhysicalAddress,
    pub length: u32,
    pub flags: DescriptorFlags,
    pub next: SplitqueueIndex<VirtqueueDescriptor>,
}

impl Default for VirtqueueDescriptor {
    fn default() -> Self {
        Self {
            address: PhysicalAddress::new(0),
            length: 0,
            flags: DescriptorFlags::NONE,
            next: SplitqueueIndex::new(0),
        }
    }
}

pub struct AvailableQueue {
    queue: DmaRegion<VirtqueueAvailable>,
}

impl AvailableQueue {
    pub fn physical_address(&self) -> PhysicalAddress {
        self.queue.physical_address()
    }

    pub fn push(&mut self, index: SplitqueueIndex<VirtqueueDescriptor>) {
        let queue_index = self.queue.index;
        let ring_index = self.queue.index % self.queue.ring.len() as u16;
        // This is likely overkill, but better to be safe than sorry!
        unsafe {
            core::ptr::write_volatile(&mut self.queue.ring[ring_index as usize], index.0);

            // From the VirtIO spec:
            // > 2.7.13.3.1 Driver Requirements: Updating idx
            // >
            // > The driver MUST perform a suitable memory barrier before the idx
            // > update, to ensure the device sees the most up-to-date copy.
            librust::mem::fence(librust::mem::FenceMode::Write);

            core::ptr::write_volatile(&mut self.queue.index, queue_index.wrapping_add(1));
        }
    }
}

#[repr(C)]
struct VirtqueueAvailable {
    flags: u16,
    index: u16,
    ring: [u16],
}

pub struct UsedQueue {
    queue: DmaRegion<VirtqueueUsed>,
    last_seen: u16,
}

impl UsedQueue {
    pub fn physical_address(&self) -> PhysicalAddress {
        self.queue.physical_address()
    }

    pub fn pop(&mut self) -> Option<VirtqueueUsedElement> {
        let index = unsafe { core::ptr::read_volatile(&self.queue.index) };
        match self.last_seen == index {
            // No new used elements
            true => None,
            false => {
                let used = unsafe {
                    core::ptr::read_volatile(&self.queue.ring[self.last_seen as usize % self.queue.ring.len()])
                };
                self.last_seen = self.last_seen.wrapping_add(1);

                Some(used)
            }
        }
    }

    pub fn drain(&mut self) -> impl Iterator<Item = VirtqueueUsedElement> + '_ {
        // FIXME: should this be a fused iterator or no?
        core::iter::from_fn(move || self.pop())
    }
}

#[repr(C)]
pub struct VirtqueueUsed {
    flags: u16,
    index: u16,
    ring: [VirtqueueUsedElement],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtqueueUsedElement {
    pub start_index: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Copy, Default)]
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
