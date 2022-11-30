// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;

assert_struct_size!(MasterBootRecord, 512);

/// A [Master Boot Record](https://en.wikipedia.org/wiki/Master_boot_record) of
/// a block device. Contains the partition table for the disk as well as
/// optional code to bootstrap the system.
#[derive(Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct MasterBootRecord {
    /// Bootstrap code area
    pub bootstrap_code: [u8; 440],
    /// OS-specifc disk signature
    pub disk_signature: [u8; 4],
    /// Unknown and unused field
    pub _unknown: [u8; 2],
    /// Partition table
    pub partitions: [PartitionEntry; 4],
    /// Boot signature
    pub boot_signature: BootSignature,
}

impl core::fmt::Debug for MasterBootRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MasterBootRecord")
            .field("disk_signature", &self.disk_signature)
            .field("partitions", &self.partitions)
            .field("boot_signature", &self.boot_signature)
            .finish_non_exhaustive()
    }
}

/// The boot signature of a Master Boot Record (`0xAA55`)
#[derive(Clone, Copy, PartialEq, Eq, PackedStruct)]
#[repr(transparent)]
pub struct BootSignature([u8; 2]);

impl BootSignature {
    /// Create a new [`BootSignature`]
    pub fn new() -> Self {
        Self([0x55, 0xAA])
    }

    /// Verify that the boot signature is valid
    pub fn verify(self) -> bool {
        self.0 == [0x55, 0xAA]
    }
}

impl core::fmt::Debug for BootSignature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.verify() {
            true => f.write_str("BootSignature(0xAA55)"),
            false => core::write!(f, "BootSignature(<invalid>={})", u16::from_le_bytes(self.0)),
        }
    }
}

impl Default for BootSignature {
    fn default() -> Self {
        Self::new()
    }
}

/// An individual partition entry of a disk
#[derive(Debug, Clone, Copy, PartialEq, Eq, PackedStruct)]
#[repr(C)]
pub struct PartitionEntry {
    /// This entry's partition status
    pub status: PartitionStatus,
    /// The first CHS address for this partition
    pub first_chs_address: ChsAddress,
    /// This entry's partition type
    pub parition_type: PartitionType,
    /// The last CHS address for this partition
    pub last_chs_address: ChsAddress,
    /// The LBA of the start of this partition
    pub lba_start: LogicalBlockAddress,
    /// The number of sectors in this partition
    pub sector_count: SectorCount,
}

/// The partition status of a [`MasterBootRecord`] parition
#[derive(Debug, Clone, Copy, PartialEq, Eq, PackedStruct)]
#[repr(transparent)]
pub struct PartitionStatus(u8);

impl PartitionStatus {
    /// Inactive partition
    pub const INACTIVE: Self = Self(0x00);
    /// Active partition
    pub const ACTIVE: Self = Self(0x80);

    /// Whether the partition is active
    pub fn is_active(self) -> bool {
        self == Self::ACTIVE
    }

    /// Whether the partition is valid
    pub fn is_valid(self) -> bool {
        self == Self::INACTIVE || self == Self::ACTIVE
    }
}

/// The type of partition
#[derive(Debug, Clone, Copy, PartialEq, Eq, PackedStruct)]
#[repr(transparent)]
pub struct PartitionType(u8);

impl PartitionType {
    /// Empty, unused partition
    pub const EMPTY: Self = Self(0x00);
    /// FAT32 (LBA)
    pub const FAT32_LBA: Self = Self(0x0C);
    /// Protective GPT MBR
    pub const GPT_PROTECTIVE_MBR: Self = Self(0xEE);
    /// EFI system partition (FAT16, FAT32, etc)
    pub const EFI_SYSTEM_PARTITION: Self = Self(0xEF);

    /// Create a new [`PartitionType`]
    pub fn new(partition_type: u8) -> Self {
        Self(partition_type)
    }

    /// Get the partition type value
    pub fn get(self) -> u8 {
        self.0
    }
}

/// A cylinder-head-sector disk address
#[derive(Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct ChsAddress([u8; 3]);

impl ChsAddress {
    /// Create a new [`ChsAddress`]
    pub fn new(cylinder: Cylinder, head: Head, sector: Sector) -> Self {
        // Top 2 bits of the `sector` are used for the cylinder
        let sector = (((cylinder.0 >> 8) as u8 & 0x3) << 6) | sector.0;

        Self([head.0, sector, cylinder.0 as u8])
    }

    /// Get the individual cylinder, head, and sector values
    pub fn to_parts(self) -> (Cylinder, Head, Sector) {
        let [head, sector, cylinder] = self.0;
        let mut cylinder = u16::from(cylinder);

        cylinder |= ((sector >> 6) as u16) << 8;

        (Cylinder::new(cylinder), Head::new(head), Sector::new(sector))
    }
}

impl core::fmt::Debug for ChsAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{:?}", self.to_parts())
    }
}

/// A disk cylinder number
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct Cylinder(u16);

impl Cylinder {
    /// Create a new [`Cylinder`]. The max cylinder value is `1023` and values
    /// larger than that will be truncated.
    pub fn new(cylinder: u16) -> Self {
        Self(cylinder & 0x3FF)
    }

    /// Get the cylinder number
    pub fn get(self) -> u16 {
        self.0
    }
}

/// A disk head number
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct Head(u8);

impl Head {
    /// Create a new [`Head`]
    pub fn new(head: u8) -> Self {
        Self(head)
    }

    /// Get the head number
    pub fn get(self) -> u8 {
        self.0
    }
}

/// A disk sector number
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct Sector(u8);

impl Sector {
    /// Create a new [`Sector`]. The max sector value is `63` and values larger
    /// than that will be truncated.
    pub fn new(sector: u8) -> Self {
        Self(sector & 0x3F)
    }

    /// Get the sector number
    pub fn get(self) -> u8 {
        self.0
    }
}

/// A logical block address (LBA). Represents the number of block sized chunks
/// into a disk.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LogicalBlockAddress([u8; 4]);

impl LogicalBlockAddress {
    /// Create a new [`LogicalBlockAddress`]
    pub fn new(lba: u32) -> Self {
        Self(lba.to_ne_bytes())
    }

    /// Get the LBA value
    pub fn get(self) -> u32 {
        u32::from_ne_bytes(self.0)
    }
}

impl core::fmt::Debug for LogicalBlockAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "LogicalBlockAddress({})", self.get())
    }
}

/// A logical block address (LBA). Represents the number of block sized chunks
/// into a disk.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct SectorCount([u8; 4]);

impl SectorCount {
    /// Create a new [`SectorCount`]
    pub fn new(count: u32) -> Self {
        Self(count.to_ne_bytes())
    }

    /// Get the LBA value
    pub fn get(self) -> u32 {
        u32::from_ne_bytes(self.0)
    }
}

impl core::fmt::Debug for SectorCount {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "SectorCount({})", self.get())
    }
}
