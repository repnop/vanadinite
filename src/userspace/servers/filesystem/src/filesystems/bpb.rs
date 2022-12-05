// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::partitions::mbr::BootSignature;
use alchemy::PackedStruct;

/// The BIOS Parameter Block. Contains information relevant for filesystems
/// including, but not limited to, the FAT12, FAT16, and FAT32 filesystem
/// family. Only FAT32 fields are currently supported.
#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct BiosParameterBlock {
    /// Jump instruction to boot code
    pub jmp_boot: [u8; 3],
    /// Eight character OEM name
    pub oem_name: [u8; 8],
    /// Bytes per sector. Possible values are: 512, 1024, 2048, or 4096.
    pub bytes_per_sector: UnalignedU16,
    /// Sectors per cluster. Must be greater than zero and a power of two.
    pub sectors_per_cluster: u8,
    /// Number of reserved sectors in the reserved region of the volume starting
    /// at the first sector of the volume. Used to align the data area to a
    /// multiple of the cluster size. Must not be zero.
    pub reserved_sector_count: UnalignedU16,
    /// Number of FATs in the volume
    pub num_fats: u8,
    /// Root entry count for FAT12/16 formatted partitions
    pub fat16_root_entry_count: UnalignedU16,
    /// Total number of sectors for FAT12/16 formatted partitions
    pub fat16_total_sectors: UnalignedU16,
    /// Media type
    pub media: u8,
    /// FAT size in sectors for FAT12/16 formatted partitions
    pub fat16_fat_size: UnalignedU16,
    /// Sectors per track for geometrical disks
    pub sectors_per_track: UnalignedU16,
    /// Number of heads for geometrical disks
    pub num_heads: UnalignedU16,
    /// Number of hidden sectors preceding the partition data
    pub num_hidden_sectors: UnalignedU32,
    /// Total number of sectors for FAT32 formatted partitions
    pub fat32_total_sectors: UnalignedU32,
    /* Begin Extended BPB */
    /// FAT32 FAT size in sectors
    pub fat32_fat_size: UnalignedU32,
    /// Extended flags
    pub extended_flags: ExtendedFlags,
    /// Filesystem version. Must be zero.
    pub fs_version: UnalignedU16,
    /// Cluster number of the first cluster of the root directory. Value should
    /// be 2 or the first non-bad cluster available.
    pub root_cluster: UnalignedU32,
    /// Sector number of the `FSINFO` structure in the reserved area of the
    /// FAT32 volume. Value is usually 1.
    pub fs_info_sector: UnalignedU16,
    /// Sector number of the reserved area for the backup copy of the boot
    /// record. Must be 0 or 6.
    pub backup_boot_sector: UnalignedU16,
    #[allow(missing_docs)]
    pub _reserved: [u8; 12],
    /// x86 `int 0x32` drive number. Value is `0x80` or `0x00`
    pub drive_number: u8,
    #[allow(missing_docs)]
    pub _reserved1: u8,
    /// If the following three fields are set, this field has a value of `0x29`.
    /// Otherwise, the fields are not available.
    pub extended_boot_signature: u8,
    /// Volume serial number
    pub volume_serial: UnalignedU32,
    /// A reflection of the 11-byte volume label in the root directory of the
    /// filesystem
    pub volume_label: [u8; 11],
    /// `FAT32   `
    pub filesystem_type: [u8; 8],
    #[allow(missing_docs)]
    pub _empty: [u8; 420],
    /// Boot signature
    pub signature_word: BootSignature,
}

/// An unaligned `u16`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct UnalignedU16([u8; 2]);

impl UnalignedU16 {
    #[allow(missing_docs)]
    pub fn new(n: u16) -> Self {
        Self(n.to_le_bytes())
    }

    #[allow(missing_docs)]
    pub fn get(self) -> u16 {
        u16::from_le_bytes(self.0)
    }
}

/// An unaligned `u32`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct UnalignedU32([u8; 4]);

impl UnalignedU32 {
    #[allow(missing_docs)]
    pub fn new(n: u32) -> Self {
        Self(n.to_le_bytes())
    }

    #[allow(missing_docs)]
    pub fn get(self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

/// FAT32 extended flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct ExtendedFlags(UnalignedU16);

impl ExtendedFlags {
    #[allow(missing_docs)]
    pub fn new(active_fat: u8, fat_mirroring: bool) -> Self {
        let val = ((active_fat & 0xF) | (u8::from(fat_mirroring) << 7)) as u16;
        Self(UnalignedU16::new(val))
    }

    /// FAT number of the active FAT if mirroring is disabled
    pub fn active_fat(self) -> u8 {
        (self.0.get() as u8) & 0xF
    }

    /// Whether changes to a FAT are mirrored to all other FATs
    pub fn fat_mirroring(self) -> bool {
        (self.0.get() as u8) & 0x80 == 0x00
    }
}
