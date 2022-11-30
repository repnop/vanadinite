// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;
use endian::{LittleEndianU16, LittleEndianU32};

/// Error that occurred when parsing a GUID
#[derive(Debug)]
pub enum GuidParseError {
    /// An invalid character was encountered (not hexadecimal or `'`)
    InvalidCharacter,
    /// The GUID string has an invalid length (must be 36 characters)
    InvalidLength,
    /// Invalid character found where a separator (`-`) was expected
    InvalidSeparator,
}

/// Convenience macro for const parsing a GUID string literal into a [`Guid`]
#[macro_export]
macro_rules! guid {
    ($s:literal) => {{
        const { $crate::partitions::gpt::guid::unwrap($crate::partitions::gpt::Guid::parse($s)) }
    }};
}

#[doc(hidden)]
pub const fn unwrap(r: Result<Guid, GuidParseError>) -> Guid {
    match r {
        Ok(g) => g,
        Err(_) => panic!("Invalid GUID literal string"),
    }
}

/// An EFI GUID (Globally Unique Idneitifer)
#[derive(Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(C)]
pub struct Guid(LittleEndianU32, LittleEndianU16, LittleEndianU16, u8, u8, [u8; 6]);

impl Guid {
    /// Unused partition
    pub const UNUSED: Self = guid!("00000000-0000-0000-0000-000000000000");
    /// EFI system partition (FAT16, FAT32, etc)
    pub const EFI_SYSTEM_PARTITION: Self = guid!("C12A7328-F81F-11D2-BA4B-00A0C93EC93B");
    /// Partition contains a legacy Master Boot Record (see [`crate::partitions::mbr::MasterBootRecord`])
    pub const LEGACY_MBR: Self = guid!("024DEE41-33E7-11D3-9D69-0008C781F39F");
    /// Linux filesystem partition (any supported filesystem)
    pub const LINUX_FILESYSTEM_DATA: Self = guid!("0FC63DAF-8483-4772-8E79-3D69D8477DE4");

    /// Create a new [`Guid`] from its component parts
    pub const fn new(a: u32, b: u16, c: u16, d: u8, e: u8, f: [u8; 6]) -> Self {
        Self(LittleEndianU32::from_ne(a), LittleEndianU16::from_ne(b), LittleEndianU16::from_ne(c), d, e, f)
    }

    /// Parse a GUID string into a [`Guid`], returning an error if parsing fails
    #[rustfmt::skip]
    pub const fn parse(s: &str) -> Result<Self, GuidParseError> {
        macro_rules! try_hex {
            ($e:expr) => {{
                match $e {
                    Ok(n) => n,
                    Err(e) => return Err(e),
                }
            }}
        }

        const fn hex(b: u8) -> Result<u8, GuidParseError> {
            match b {
                b'0'..=b'9' => Ok(b - b'0'),
                b'a'..=b'f' => Ok(b - b'a' + 0xA),
                b'A'..=b'F' => Ok(b - b'A' + 0xA),
                _ => Err(GuidParseError::InvalidCharacter),
            }
        }

        if s.len() != 36 {
            return Err(GuidParseError::InvalidLength);
        }

        let mut a = [0u8; 4];
        let mut b = [0u8; 2];
        let mut c = [0u8; 2];
        let mut d = 0u8;
        let mut e = 0u8;
        let mut f = [0u8; 6];

        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match i {
                8 | 13 | 18 | 23 => {
                    if bytes[i] != b'-' {
                        return Err(GuidParseError::InvalidSeparator);
                    }
                }
                0 | 1 => a[3] = (a[3] << 4) | try_hex!(hex(bytes[i])),
                2 | 3 => a[2] = (a[2] << 4) | try_hex!(hex(bytes[i])),
                4 | 5 => a[1] = (a[1] << 4) | try_hex!(hex(bytes[i])),
                6 | 7 => a[0] = (a[0] << 4) | try_hex!(hex(bytes[i])),
                9 | 10 => b[1] = (b[1] << 4) | try_hex!(hex(bytes[i])),
                11 | 12 => b[0] = (b[0] << 4) | try_hex!(hex(bytes[i])),
                14 | 15 => c[1] = (c[1] << 4) | try_hex!(hex(bytes[i])),
                16 | 17 => c[0] = (c[0] << 4) | try_hex!(hex(bytes[i])),
                19 | 20 => d = (d << 4) | try_hex!(hex(bytes[i])),
                21 | 22 => e = (e << 4) | try_hex!(hex(bytes[i])),
                24 | 25 => f[0] = (f[0] << 4) | try_hex!(hex(bytes[i])),
                26 | 27 => f[1] = (f[1] << 4) | try_hex!(hex(bytes[i])),
                28 | 29 => f[2] = (f[2] << 4) | try_hex!(hex(bytes[i])),
                30 | 31 => f[3] = (f[3] << 4) | try_hex!(hex(bytes[i])),
                32 | 33 => f[4] = (f[4] << 4) | try_hex!(hex(bytes[i])),
                34 | 35 => f[5] = (f[5] << 4) | try_hex!(hex(bytes[i])),
                _ => {}
            }

            i += 1;
        }

        Ok(Self(
            LittleEndianU32::from_ne(u32::from_le_bytes(a)),
            LittleEndianU16::from_ne(u16::from_le_bytes(b)),
            LittleEndianU16::from_ne(u16::from_le_bytes(c)),
            d,
            e,
            f,
        ))
    }
}

impl core::fmt::Debug for Guid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(
            f,
            "Guid({:0<8X}-{:0<4X}-{:0<4X}-{:0<2X}{:0<2X}-{:0<2X}{:0<2X}{:0<2X}{:0<2X}{:0<2X}{:0<2X})",
            self.0.to_ne(),
            self.1.to_ne(),
            self.2.to_ne(),
            self.3,
            self.4,
            self.5[0],
            self.5[1],
            self.5[2],
            self.5[3],
            self.5[4],
            self.5[5]
        )
    }
}

impl core::fmt::Display for Guid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(
            f,
            "{:0<8X}-{:0<4X}-{:0<4X}-{:0<2X}{:0<2X}-{:0<2X}{:0<2X}{:0<2X}{:0<2X}{:0<2X}{:0<2X}",
            self.0.to_ne(),
            self.1.to_ne(),
            self.2.to_ne(),
            self.3,
            self.4,
            self.5[0],
            self.5[1],
            self.5[2],
            self.5[3],
            self.5[4],
            self.5[5]
        )
    }
}
