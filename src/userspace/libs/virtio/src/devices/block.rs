// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::VirtIoHeader;
use volatile::{Read, ReadWrite, Volatile};

#[repr(C)]
pub struct VirtIoBlockDevice {
    pub header: VirtIoHeader,
    capacity: Volatile<u64, Read>,
    size_max: Volatile<u32, Read>,
    segment_max: Volatile<u32, Read>,
    geometry: Volatile<Geometry, Read>,
    block_size: Volatile<u32, Read>,
    block_topology: Volatile<BlockTopology, Read>,
    writeback: Volatile<u8, ReadWrite>,
    _unused0: u8,
    num_queues: Volatile<u16, ReadWrite>,
    max_discard_sectors: Volatile<u32, Read>,
    max_discard_segments: Volatile<u32, Read>,
    discard_sector_alignment: Volatile<u32, Read>,
    max_write_zeroes_sectors: Volatile<u32, Read>,
    max_write_zeroes_segments: Volatile<u32, Read>,
    write_zeroes_may_unmap: Volatile<u8, Read>,
    _unused1: [u8; 3],
}

impl VirtIoBlockDevice {
    pub fn capacity(&self) -> u64 {
        self.capacity.read()
    }

    pub unsafe fn size_max(&self) -> u32 {
        self.size_max.read()
    }

    pub unsafe fn segment_max(&self) -> u32 {
        self.segment_max.read()
    }

    pub unsafe fn geometry(&self) -> Geometry {
        self.geometry.read()
    }

    pub unsafe fn block_size(&self) -> u32 {
        self.block_size.read()
    }

    pub unsafe fn block_topology(&self) -> BlockTopology {
        self.block_topology.read()
    }

    pub unsafe fn writeback(&self) -> &Volatile<u8, ReadWrite> {
        &self.writeback
    }

    pub unsafe fn num_queues(&self) -> &Volatile<u16, ReadWrite> {
        &self.num_queues
    }

    pub unsafe fn max_discard_sectors(&self) -> u32 {
        self.max_discard_sectors.read()
    }

    pub unsafe fn max_discard_segments(&self) -> u32 {
        self.max_discard_segments.read()
    }

    pub unsafe fn discard_sector_alignment(&self) -> u32 {
        self.discard_sector_alignment.read()
    }

    pub unsafe fn max_write_zeroes_sectors(&self) -> u32 {
        self.max_write_zeroes_sectors.read()
    }

    pub unsafe fn max_write_zeroes_segments(&self) -> u32 {
        self.max_write_zeroes_segments.read()
    }

    pub unsafe fn write_zeroes_may_unmap(&self) -> u8 {
        self.write_zeroes_may_unmap.read()
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Geometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BlockTopology {
    pub physical_block_exp: u8,
    pub alignment_offset: u8,
    pub suggested_io_size_in_blocks: u16,
    pub optimal_io_size_in_blocks: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct BlockDeviceFeatures(u32);

impl BlockDeviceFeatures {
    pub const SIZE_MAX: Self = Self(1 << 1);
    pub const SEGMENT_MAX: Self = Self(1 << 2);
    pub const GEOMETRY: Self = Self(1 << 4);
    pub const READ_ONLY: Self = Self(1 << 5);
    pub const BLOCK_SIZE: Self = Self(1 << 6);
    pub const FLUSH: Self = Self(1 << 9);
    pub const TOPOLOGY: Self = Self(1 << 10);
    pub const CONFIG_WRITE_CACHE_TOGGLE: Self = Self(1 << 11);
    pub const DISCARD: Self = Self(1 << 13);
    pub const WRITE_ZEROES: Self = Self(1 << 14);
}

impl core::ops::BitOr for BlockDeviceFeatures {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        BlockDeviceFeatures(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for BlockDeviceFeatures {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = BlockDeviceFeatures(self.0 | rhs.0);
    }
}

impl core::ops::BitAnd for BlockDeviceFeatures {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Command {
    pub kind: CommandKind,
    pub _reserved: u32,
    pub sector: u64,
    pub status: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum CommandKind {
    Read = 0,
    Write = 1,
    Flush = 4,
    Discard = 11,
    WriteZeroes = 13,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CommandStatus {
    Ok = 0,
    IoError = 1,
    Unsupported = 2,
}

impl CommandStatus {
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(CommandStatus::Ok),
            1 => Some(CommandStatus::IoError),
            2 => Some(CommandStatus::Unsupported),
            _ => None,
        }
    }

    pub fn into_result(self) -> Result<(), CommandError> {
        match self {
            CommandStatus::Ok => Ok(()),
            CommandStatus::IoError => Err(CommandError::IoError),
            CommandStatus::Unsupported => Err(CommandError::Unsupported),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CommandError {
    IoError,
    Unsupported,
    UnknownStatusCode(u8),
}
