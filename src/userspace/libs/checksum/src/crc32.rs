// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;

/// CRC32 memozied lookup table
pub const LOOKUP_TABLE: [u32; 256] = generate_lookup_table(0xEDB88320);

const fn generate_lookup_table(polynomial: u32) -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut value = i;
        let mut j = 0;

        while j < 8 {
            value = match value & 1 {
                1 => polynomial ^ (value >> 1),
                _ => value >> 1,
            };

            j += 1;
        }

        table[i as usize] = value;

        i += 1;
    }

    table
}

const _: () = match Crc32::calculate(b"The quick brown fox jumps over the lazy dog").get() {
    0x414FA339 => {}
    _ => panic!("`Crc32::calculate` produced an incorrect checksum!"),
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PackedStruct)]
#[repr(transparent)]
pub struct Crc32(u32);

impl Crc32 {
    /// Create a new [`Crc32`] from a previous computed checksum
    pub const fn new(checksum: u32) -> Self {
        Self(checksum)
    }

    /// Calculate the CRC32 checksum of a byte slice
    pub const fn calculate(data: &[u8]) -> Self {
        let mut checksum = u32::MAX;
        let mut i = 0;

        while i < data.len() {
            let byte = data[i];
            let table_index = ((checksum as u8) ^ byte) as usize;
            checksum = (checksum >> 8) ^ LOOKUP_TABLE[table_index];
            i += 1;
        }

        Self(!checksum)
    }

    /// Get the underlying checksum value
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl core::fmt::Debug for Crc32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "Crc32({:#X})", self.0)
    }
}
