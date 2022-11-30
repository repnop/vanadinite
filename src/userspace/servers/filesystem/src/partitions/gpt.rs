// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

/// Utilities for working with EFI GUIDs
pub mod guid;

use alchemy::PackedStruct;
use checksum::crc32::Crc32;
use endian::{BigEndianU32, LittleEndianU32, LittleEndianU64};
pub use guid::Guid;

assert_struct_size!(GptHeader, 96);

/// The GUID Partition Table header
#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct GptHeader {
    /// GPT header signature, must be `b"EFI PART"` encoded as a little-endian
    /// `u64`
    pub signature: GptSignature,
    /// Revision version of this header, currently must be 1.0 (`0x00010000`)
    pub revision: GptHeaderRevision,
    /// Size in bytes of the header structure
    pub header_size: LittleEndianU32,
    /// CRC32 checksum of this header
    pub header_checksum: Crc32,
    /// Reserved
    pub _reserved: ZeroedU32,
    /// LBA of this header
    pub my_lba: LittleEndianU64,
    /// LBA of the alternative GPT header
    pub alternate_header_lba: LittleEndianU64,
    /// First usable LBA that can be used by a partition
    pub first_usable_lba: LittleEndianU64,
    /// Last usable LBA that can be used by a partition
    pub last_usable_lba: LittleEndianU64,
    /// GUID to uniquely identify the disk
    pub disk_guid: Guid,
    /// Partition table information
    pub partition_table_info: GptPartitionTableInfo,
}

/// The GPT signature (`b"EFI PART"` encoded as a little-endian `u64`)
#[derive(Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct GptSignature(LittleEndianU64);

impl GptSignature {
    /// Create a new [`GptSignature`]
    pub const fn new() -> Self {
        Self(LittleEndianU64::from_ne(0x5452415020494645))
    }

    /// Whether the signature matches the expected value
    pub const fn verify(self) -> bool {
        self.0.to_ne() == 0x5452415020494645
    }
}

impl core::fmt::Debug for GptSignature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.verify() {
            true => f.write_str("GptSignature(\"EFI PART\")"),
            false => core::write!(f, "GptSignature(<invalid>={})", self.0.to_ne()),
        }
    }
}

const _: () = match GptHeaderRevision::new(1, 2).to_major_minor() {
    (1, 2) => {}
    _ => panic!("Incorrect `GptHeaderRevision` encoding"),
};

/// The GPT header revision
#[derive(Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct GptHeaderRevision(LittleEndianU32);

impl GptHeaderRevision {
    /// Create a new [`GptHeaderRevision`]
    pub const fn new(major: u16, minor: u16) -> Self {
        let major = (major as u32) << 16;
        Self(LittleEndianU32::from_ne(major | minor as u32))
    }

    /// Extract the major and minor revision numbers
    pub const fn to_major_minor(self) -> (u16, u16) {
        let major = (self.0.to_ne() >> 16) as u16;
        let minor = self.0.to_ne() as u16;

        (major, minor)
    }
}

impl core::fmt::Debug for GptHeaderRevision {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (major, minor) = self.to_major_minor();
        core::write!(f, "GptHeaderRevision(v{}.{})", major, minor)
    }
}

assert_struct_size!(GptPartitionTableInfo, 24);

/// Information about the GPT partition table
#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct GptPartitionTableInfo {
    /// Starting LBA for partition table entries
    pub entry_start_lba: LittleEndianU64,
    /// Number of entries
    pub entry_count: LittleEndianU32,
    /// Size of each individual partition entry
    pub entry_size: LittleEndianU32,
    /// Partition table entries checksum
    pub entry_table_checksum: Crc32,
    /// Padding for alignment
    pub _padding: ZeroedU32,
}

assert_struct_size!(GptPartitionEntry, 128);

/// A single GPT partition entry
#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct GptPartitionEntry {
    /// Partition type GUID
    pub type_guid: Guid,
    /// Unique GUID of the partition described by this entry
    pub unique_guid: Guid,
    /// First LBA of the partition
    pub start_lba: LittleEndianU64,
    /// Last LBA of the partition
    pub end_lba: LittleEndianU64,
    /// Reserved attribute bits
    pub attributes: GptPartitionAttributes,
    /// Partition name
    pub name: GptPartitionName,
}

/// Possible errors encountered while trying to read a partition name as a UTF-8
/// string
pub enum PartitionNameError {
    /// Invalid UTF-8 was encountered
    InvalidUtf8(core::str::Utf8Error),
    /// No terminating null byte was found
    NoTerminatingNull,
}

#[allow(dead_code)]
const fn is_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }

        i += 1;
    }

    true
}

const _: () = match GptPartitionName::new("foobar").as_str() {
    Ok(s) if is_eq(s.as_bytes(), b"foobar") => {}
    _ => panic!("Something has gone horribly wrong"),
};

/// A GPT partition entry name
#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct GptPartitionName([u8; 72]);

impl GptPartitionName {
    /// Create a new [`GptPartitionName`] from a `&str`. Note that the name will
    /// be truncated to 71 bytes if the string is longer than that as GPT
    /// partition names have a max length of 72 characters, and one must be reserved for
    pub const fn new(name: &str) -> Self {
        // FIXME: replace with `str::floor_char_boundary` once its available in
        // const context
        const fn floor_char_boundary(s: &str, index: usize) -> usize {
            const fn is_utf8_char_boundary(b: u8) -> bool {
                // This is bit magic equivalent to: b < 128 || b >= 192
                (b as i8) >= -0x40
            }

            if index >= s.len() {
                s.len()
            } else {
                let lower_bound = index.saturating_sub(3);
                let mut new_index = index;

                while new_index >= lower_bound {
                    if is_utf8_char_boundary(s.as_bytes()[new_index]) {
                        break;
                    }

                    new_index -= 1;
                }

                new_index
            }
        }

        let mut bytes = [0u8; 72];
        let end_index = floor_char_boundary(name, 70 /* 71 is null byte */);

        let mut i = 0;
        while i < end_index {
            bytes[i] = name.as_bytes()[i];
            i += 1;
        }

        Self(bytes)
    }

    /// Overwrite the current [`GptPartitionName`] with a new name. The same
    /// truncation limitations apply as in [`GptPartitionName::new`]
    pub fn overwrite(&mut self, name: &str) {
        *self = Self::new(name);
    }

    /// Try to parse the partition name as a UTF-8 string
    pub const fn as_str(&self) -> Result<&str, PartitionNameError> {
        let mut null_byte_index = None;
        let mut i = 0;

        while i < self.0.len() {
            if self.0[i] == b'\0' {
                null_byte_index = Some(i);
                break;
            }

            i += 1;
        }

        match null_byte_index {
            // FIXME: this doesn't need to be unsafe but slicing is not const-available yet.
            // Safety: we know `i` never exceeds the array length
            Some(index) => match core::str::from_utf8(unsafe { core::slice::from_raw_parts(self.0.as_ptr(), index) }) {
                Ok(s) => Ok(s),
                Err(e) => Err(PartitionNameError::InvalidUtf8(e)),
            },
            None => Err(PartitionNameError::NoTerminatingNull),
        }
    }

    /// A byte-view of the partition name in the case which it is not encoded as
    /// UTF-8
    pub const fn as_bytes(&self) -> &[u8] {
        let mut null_byte_index = None;
        let mut i = 0;

        while i < self.0.len() {
            if self.0[i] == b'\0' {
                null_byte_index = Some(i);
                break;
            }

            i += 1;
        }

        match null_byte_index {
            // FIXME: this doesn't need to be unsafe but slicing is not const-available yet.
            // Safety: we know `i` never exceeds the array length
            Some(index) => unsafe { core::slice::from_raw_parts(self.0.as_ptr(), index) },
            None => unsafe { core::slice::from_raw_parts(self.0.as_ptr(), 72) },
        }
    }
}

/// GPT partition attributes
#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct GptPartitionAttributes(u64);

impl GptPartitionAttributes {
    /// No attributes
    pub const NONE: Self = Self(0);
    /// Required partition for the system to function
    pub const REQUIRED_PARTITION: Self = Self(1 << 0);
    /// If set, the firmware must not produce an `EFI_BLOCK_IO_PROTOCOL` for
    /// this partition.
    pub const NO_BLOCK_IO_PROTOCOL: Self = Self(1 << 1);
    /// The partition may be bootable by a legacy BIOS
    pub const LEGACY_BIOS_BOOTABLE: Self = Self(1 << 2);

    /// Create a new [`GptPartitionAttributes`] from the standard attributes and
    /// GUID-specific attributes.
    pub const fn new(attributes: Self, guid_specific_use: u16) -> Self {
        Self(attributes.0 | ((guid_specific_use as u64) << 48))
    }

    /// Set the standard attributes for this partition
    pub fn set_attributes(&mut self, attributes: Self) {
        self.0 &= !0x7;
        self.0 |= attributes.0;
    }

    /// Set the GUID-specific use bits of the partition
    pub fn set_guid_specific_use(&mut self, guid_specific_use: u16) {
        self.0 &= !(0xFFFF << 48);
        self.0 |= u64::from(guid_specific_use) << 48;
    }
}

#[allow(missing_docs)]
#[derive(Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct ZeroedU32(u32);

impl ZeroedU32 {
    #[allow(missing_docs)]
    pub const fn new() -> Self {
        Self(0)
    }
}

impl core::fmt::Debug for ZeroedU32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ZeroedU32(..)")
    }
}
