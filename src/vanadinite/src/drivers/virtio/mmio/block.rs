// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::common::{DeviceType, StatusFlag, VirtIoHeader};
use crate::{
    drivers::virtio::queue::SplitVirtqueue,
    mem::fence,
    utils::volatile::{Read, ReadWrite, Volatile},
};

#[repr(C)]
pub struct VirtIoBlockDevice {
    header: VirtIoHeader,
    capacity: Volatile<u64, Read>,
    size_max: Volatile<u32, Read>,
    segment_max: Volatile<u32, Read>,
    geometry: Volatile<Geometry, Read>,
    block_size: Volatile<u32, Read>,
    block_topology: Volatile<BlockTopology, Read>,
    writeback: Volatile<u8, ReadWrite>,
    _unused0: [u8; 3],
    max_discard_sectors: Volatile<u32, Read>,
    max_discard_segments: Volatile<u32, Read>,
    discard_sector_alignment: Volatile<u32, Read>,
    max_write_zeroes_sectors: Volatile<u32, Read>,
    max_write_zeroes_segments: Volatile<u32, Read>,
    write_zeroes_may_unmap: Volatile<u8, Read>,
    _unused1: [u8; 3],
}

impl VirtIoBlockDevice {
    pub fn init(&mut self, queue: &SplitVirtqueue, queue_select: u32) -> Result<(), &'static str> {
        // TODO: memory barriers??
        self.header.status.reset();
        self.header.status.set_flag(StatusFlag::Acknowledge);
        self.header.status.set_flag(StatusFlag::Driver);
        // TODO: maybe use feature bits at some point
        let _ = self.features();
        // No features
        self.header.driver_features_select.write(0);
        self.header.status.set_flag(StatusFlag::FeaturesOk);

        if !self.header.status.is_set(StatusFlag::FeaturesOk) {
            return Err("some set of features not understood");
        }

        self.header.queue_select.write(queue_select);
        self.header.queue_size.write(queue.queue_size());
        self.header.queue_descriptor.set(queue.descriptor_physical_address());
        self.header.queue_available.set(queue.available_physical_address());
        self.header.queue_used.set(queue.used_physical_address());
        self.header.queue_ready.ready();

        fence();

        self.header.status.set_flag(StatusFlag::DriverOk);

        Ok(())
    }

    //pub fn read(&mut self, queue: &mut SplitVirtqueue, command: Command, data: &mut [u8; 512]) -> Result<(), ()> {}

    pub fn features(&self) -> u32 {
        self.header.features()
    }
}

#[repr(C)]
pub struct Geometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

#[repr(C)]
pub struct BlockTopology {
    pub physical_block_exp: u8,
    pub alignment_offset: u8,
    pub suggested_io_size_in_blocks: u16,
    pub optimal_io_size_in_blocks: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum FeatureBits {
    SizeMax = 1 << 1,
    SegmentMax = 1 << 2,
    Geometry = 1 << 4,
    ReadOnly = 1 << 5,
    BlockSize = 1 << 6,
    Flush = 1 << 9,
    Topology = 1 << 10,
    ConfigWriteCacheToggle = 1 << 11,
    Discard = 1 << 13,
    WriteZeroes = 1 << 14,
}

impl core::ops::BitAnd<FeatureBits> for u32 {
    type Output = bool;

    fn bitand(self, rhs: FeatureBits) -> Self::Output {
        self & (rhs as u32) == (rhs as u32)
    }
}

impl core::ops::BitAnd<u32> for FeatureBits {
    type Output = bool;

    fn bitand(self, rhs: u32) -> Self::Output {
        rhs & (self as u32) == (self as u32)
    }
}

pub struct Command {
    kind: CommandKind,
    _reserved: u32,
    sector: u64,
    status: u8,
}

#[derive(Debug)]
#[repr(u32)]
pub enum CommandKind {
    Read = 0,
    Write = 1,
    Flush = 4,
    Discard = 11,
    WriteZeroes = 13,
}
