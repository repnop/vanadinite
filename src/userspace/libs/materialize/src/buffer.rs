// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::MINIMUM_ALIGNMENT;
use alloc::alloc::Global;
use core::{
    alloc::{Allocator, Layout},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
use librust::{mem::SharedMemoryAllocation, syscalls::mem::MemoryPermissions, units::Bytes};

pub struct AlignedBuffer<MODE: BufferMode> {
    buffer: MODE::Buffer,
}

pub trait Buffer: Deref<Target = [u8]> + DerefMut {
    type Error;
    fn resize(&mut self, new_len: usize, value: u8) -> Result<(), Self::Error>;
}

pub trait BufferMode {
    type Buffer;
}

pub struct SharableMemory;

pub struct MemoryCapabilityBuffer {
    mem: SharedMemoryAllocation,
    len: usize,
}

impl MemoryCapabilityBuffer {
    const MAX_CAPACITY: usize = isize::MAX as usize;

    pub fn new(starting_capacity: usize) -> Result<Self, librust::error::SyscallError> {
        let mem =
            SharedMemoryAllocation::new(Bytes(starting_capacity), MemoryPermissions::READ | MemoryPermissions::WRITE)?;

        Ok(Self { mem, len: 0 })
    }

    pub fn resize(&mut self, new_len: usize, value: u8) -> Result<(), librust::error::SyscallError> {
        if new_len <= self.len {
            self.len = new_len;
            return Ok(());
        }

        self.realloc(new_len)?;
        // Safety: we always allocate zeroed memory so the buffer is always fully
        // initialized
        unsafe { self.mem.as_mut()[self.len..new_len].fill(value) };
        self.len = new_len;

        Ok(())
    }

    fn realloc(&mut self, new_len: usize) -> Result<(), librust::error::SyscallError> {
        if new_len <= self.capacity() {
            return Ok(());
        }

        let capacity = new_len.next_power_of_two().min(Self::MAX_CAPACITY);
        let mut new_buffer =
            SharedMemoryAllocation::new(Bytes(capacity), MemoryPermissions::READ | MemoryPermissions::WRITE)?;

        unsafe { new_buffer.as_mut()[..self.len].copy_from_slice(&self.mem.as_ref()[..self.len]) };
        // TODO: free old capability memory when that's a thing

        self.mem = new_buffer;

        Ok(())
    }

    fn capacity(&self) -> usize {
        unsafe { self.mem.as_ref().len() }
    }
}

impl Deref for MemoryCapabilityBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { &self.mem.as_ref()[..self.len] }
    }
}

impl DerefMut for MemoryCapabilityBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.mem.as_mut()[..self.len] }
    }
}

pub struct OwnedHeap;
impl BufferMode for OwnedHeap {
    type Buffer = AlignedHeapBuffer;
}

pub struct AlignedHeapBuffer {
    ptr: NonNull<[u8]>,
    len: usize,
    cap: usize,
}

impl AlignedHeapBuffer {
    const MAX_CAPACITY: usize = isize::MAX as usize;

    pub const fn new() -> Self {
        Self {
            ptr: unsafe {
                NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(MINIMUM_ALIGNMENT as *mut u8, 0))
            },
            len: 0,
            cap: 0,
        }
    }

    pub fn resize(&mut self, new_len: usize, value: u8) -> Result<(), core::alloc::AllocError> {
        if new_len <= self.len {
            self.len = new_len;
            return Ok(());
        }

        self.realloc(new_len)?;
        // Safety: we always use `allocate_zeroed` so the buffer is always fully
        // initialized
        unsafe { self.ptr.as_mut()[self.len..new_len].fill(value) };
        self.len = new_len;

        Ok(())
    }

    fn realloc(&mut self, new_len: usize) -> Result<(), core::alloc::AllocError> {
        if new_len <= self.cap {
            return Ok(());
        }

        match self.cap {
            0 => self.init(new_len.next_power_of_two()),
            cap => {
                let capacity = new_len.next_power_of_two().min(Self::MAX_CAPACITY);
                let mut new_buffer = unsafe {
                    Global.allocate_zeroed(Layout::from_size_align_unchecked(capacity, crate::MINIMUM_ALIGNMENT))?
                };

                unsafe { new_buffer.as_mut()[..self.len].copy_from_slice(&self.ptr.as_ref()[..self.len]) };
                unsafe {
                    Global.deallocate(
                        self.ptr.as_non_null_ptr(),
                        Layout::from_size_align_unchecked(cap, crate::MINIMUM_ALIGNMENT),
                    )
                };

                self.ptr = new_buffer;
                self.cap = capacity;

                Ok(())
            }
        }
    }

    #[cold]
    fn init(&mut self, starting_capacity: usize) -> Result<(), core::alloc::AllocError> {
        if starting_capacity > Self::MAX_CAPACITY {
            return Err(core::alloc::AllocError);
        }

        unsafe {
            self.ptr = Global
                .allocate_zeroed(Layout::from_size_align_unchecked(starting_capacity, crate::MINIMUM_ALIGNMENT))?
        };
        self.cap = starting_capacity;

        Ok(())
    }
}

impl Buffer for AlignedHeapBuffer {
    type Error = core::alloc::AllocError;

    fn resize(&mut self, new_len: usize, value: u8) -> Result<(), Self::Error> {
        self.resize(new_len, value)
    }
}

impl Deref for AlignedHeapBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self.cap {
            0 => &[],
            _ => unsafe { &self.ptr.as_ref()[..self.len] },
        }
    }
}

impl DerefMut for AlignedHeapBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.cap {
            0 => &mut [],
            _ => unsafe { &mut self.ptr.as_mut()[..self.len] },
        }
    }
}

impl Drop for AlignedHeapBuffer {
    fn drop(&mut self) {
        unsafe {
            Global.deallocate(
                self.ptr.as_non_null_ptr(),
                Layout::from_size_align_unchecked(self.cap, crate::MINIMUM_ALIGNMENT),
            )
        };
    }
}

impl core::fmt::Debug for AlignedHeapBuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        (**self).fmt(f)
    }
}

// pub struct OwnedStack<const SIZE: usize>;
// impl<const SIZE: usize> BufferMode for OwnedStack<SIZE> {
//     type Buffer<'a> = OwnedStackBuffer<SIZE>;
// }

// pub struct Borrowed;
// impl BufferMode for Borrowed {
//     type Buffer<'a> = BorrowedBuffer<'a>;
// }

// pub struct BorrowedBuffer<'a> {
//     buffer: &'a [u8],
// }

// impl<'a> BorrowedBuffer<'a> {
//     pub fn new(buffer: &'a [u64]) -> Self {
//         Self {
//             buffer: unsafe {
//                 &*(core::slice::from_raw_parts(buffer.as_ptr().cast(), buffer.len() * core::mem::size_of::<u64>()))
//             },
//         }
//     }
// }
