// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

/// VirtIO block device driver
pub mod virtio;

use crate::BoxedFuture;
use units::data::Bytes;

/// An error representing possible device operation errors
#[derive(Debug)]
pub enum DeviceError {
    /// An error reading from the device
    ReadError,
    /// An error writing from the device
    WriteError,
}

/// An index representing the data stored on a block device at `sector_index *
/// block_size` bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectorIndex(u64);

impl SectorIndex {
    /// Create a new [`SectorIndex`]
    pub fn new(sector_index: u64) -> Self {
        Self(sector_index)
    }

    /// Get the sector index value
    pub fn get(self) -> u64 {
        self.0
    }
}

impl core::ops::Add for SectorIndex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Add<u64> for SectorIndex {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::Add<SectorIndex> for u64 {
    type Output = SectorIndex;

    fn add(self, rhs: SectorIndex) -> Self::Output {
        SectorIndex(self + rhs.0)
    }
}

impl core::ops::AddAssign<u64> for SectorIndex {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::AddAssign<SectorIndex> for SectorIndex {
    fn add_assign(&mut self, rhs: SectorIndex) {
        self.0 += rhs.0;
    }
}

/// A block device owned buffer that can be modified for use by [`crate::filesystems::Filesystem`]s
/// for reading and writing chunks of data in a zero-copy fashion
#[must_use = "Allocating a `DataBlock` and not using it is expensive and wasteful"]
pub struct DataBlock {
    private: usize,
    ptr: *mut [u8],
    drop: fn(usize, *mut [u8]),
}

impl DataBlock {
    /// Create a new [`DataBlock`] via a pointer to a byte slice and a callback
    /// to run on drop
    ///
    /// # Safety
    ///
    /// `ptr` must point to a valid, initialized slice of bytes aligned to at
    /// least 8 bytes that is both readable and writable, and will not cause
    /// reference aliasing when used by the consumer of the [`DataBlock`]
    pub unsafe fn new(private: usize, ptr: *mut [u8], drop: fn(usize, *mut [u8])) -> Self {
        Self { private, ptr, drop }
    }

    /// Leak the underlying pointer, returning it and not running the drop
    /// callback. Meant to be used by the [`BlockDevice`] implementations
    /// themselves.
    pub fn leak(this: Self) -> (usize, *mut [u8]) {
        let (private, ptr) = (this.private, this.ptr);
        core::mem::forget(this);

        (private, ptr)
    }
}

impl core::ops::Drop for DataBlock {
    fn drop(&mut self) {
        (self.drop)(self.private, self.ptr)
    }
}

impl core::ops::Deref for DataBlock {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        // Safety: this is safe as per the contract on `DataBlock::new`
        unsafe { &*self.ptr }
    }
}

impl core::ops::DerefMut for DataBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: this is safe as per the contract on `DataBlock::new`
        unsafe { &mut *self.ptr }
    }
}

impl core::fmt::Debug for DataBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{:?}", &**self)
    }
}

unsafe impl Send for DataBlock {}
unsafe impl Sync for DataBlock {}

/// A block device that can be read and potentially written to
pub trait BlockDevice: Send + Sync {
    /// The size in bytes for the block size of the device
    fn block_size(&self) -> Bytes;
    /// Handle a pending interrupt
    fn handle_interrupt(&self);
    /// Whether the device is read-only
    fn read_only(&self) -> bool;

    /// Allocate a [`DataBlock`] that users can write to for later storage on the device
    fn alloc_data_block(&self) -> BoxedFuture<'static, DataBlock>;
    /// Attempt to trigger a flush of any block caches the device may have of
    /// the specified [`SectorIndex`] range
    fn flush(&self, range: core::ops::Range<SectorIndex>) -> BoxedFuture<'static, ()>;
    /// Try to read a [`DataBlock`] from the block device at the specified [`SectorIndex`]
    fn read(&self, sector: SectorIndex) -> BoxedFuture<'static, Result<DataBlock, DeviceError>>;
    /// Try to write a [`DataBlock`] to the specified [`SectorIndex`]
    fn write(&self, sector: SectorIndex, block: DataBlock) -> BoxedFuture<'static, Result<(), DeviceError>>;
}
