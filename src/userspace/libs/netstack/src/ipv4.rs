// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{BufferTooSmall, Length16};
use alchemy::PackedStruct;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpV4Socket {
    pub ip: IpV4Address,
    pub port: u16,
}

impl IpV4Socket {
    pub fn new(ip: IpV4Address, port: u16) -> Self {
        Self { ip, port }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct IpV4Address([u8; 4]);

impl IpV4Address {
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub fn to_bytes(self) -> [u8; 4] {
        self.0
    }
}

impl From<[u8; 4]> for IpV4Address {
    fn from(b: [u8; 4]) -> Self {
        Self::new(b[0], b[1], b[2], b[3])
    }
}

impl core::fmt::Display for IpV4Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

pub struct IpV4AddressParseErr;
impl core::str::FromStr for IpV4Address {
    type Err = IpV4AddressParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = [0u8; 4];

        for (i, part) in s.split('.').enumerate() {
            if i > 3 {
                return Err(IpV4AddressParseErr);
            }

            parts[i] = part.parse().map_err(|_| IpV4AddressParseErr)?;
        }

        Ok(Self(parts))
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct IpV4Header {
    pub version_ihl: VersionIhl,
    pub dscp_ecn: DscpEcn,
    pub len: Length16,
    pub identification: Identification,
    pub flags_fragment_offset: FlagsFragmentOffset,
    pub ttl: u8,
    pub protocol: Protocol,
    pub header_checksum: IpV4HeaderChecksum,
    pub source_ip: IpV4Address,
    pub destination_ip: IpV4Address,
}

impl IpV4Header {
    pub fn split_slice_ref(slice: &[u8]) -> Result<(&IpV4Header, &[u8]), BufferTooSmall> {
        if slice.len() < core::mem::size_of::<Self>() {
            return Err(BufferTooSmall);
        }

        let (header, payload) = slice.split_array_ref::<{ core::mem::size_of::<Self>() }>();
        Ok((Self::from_bytes_ref(header), payload))
    }

    pub fn split_slice_mut(slice: &mut [u8]) -> Result<(&mut IpV4Header, &mut [u8]), BufferTooSmall> {
        if slice.len() < core::mem::size_of::<Self>() {
            return Err(BufferTooSmall);
        }

        let (header, payload) = slice.split_array_mut::<{ core::mem::size_of::<Self>() }>();
        Ok((Self::from_bytes_mut(header), payload))
    }

    pub fn generate_checksum(&mut self) {
        let mut checksum = 0u16;

        let iter = self.as_bytes().array_chunks::<2>().copied().enumerate();
        for (i, bytes) in iter {
            // Skip checksum
            if i == 10 {
                continue;
            }

            let n = u16::from_be_bytes(bytes);
            let (new_checksum, overflow) = checksum.overflowing_add(n);
            match overflow {
                true => checksum = new_checksum.overflowing_add(1).0,
                false => checksum = new_checksum,
            }
        }

        self.header_checksum.set(!checksum);
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct VersionIhl(u8);

impl VersionIhl {
    pub fn new() -> Self {
        // Version = 4
        // IHL = 5 (TODO: allow options)
        Self(0x45)
    }
}

impl Default for VersionIhl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct DscpEcn(u8);

impl DscpEcn {
    pub fn new() -> Self {
        Self(0)
    }
}

impl Default for DscpEcn {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct Identification([u8; 2]);

impl Identification {
    pub fn new() -> Self {
        Self([0; 2])
    }
}

impl Default for Identification {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct FlagsFragmentOffset([u8; 2]);

impl FlagsFragmentOffset {
    pub fn new(flag: Flag, fragment_offset: u16) -> Self {
        let value = (fragment_offset << 3) | flag.0 as u16;
        Self(value.to_be_bytes())
    }
}

pub struct Flag(u8);

impl Flag {
    pub const NONE: Self = Self(0);
    pub const DONT_FRAGMENT: Self = Self(1 << 1);
    pub const MORE_FRAGMENTS: Self = Self(1 << 2);
}

impl core::ops::BitAnd<Flag> for FlagsFragmentOffset {
    type Output = bool;

    fn bitand(self, rhs: Flag) -> Self::Output {
        (self.0[0] & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct Protocol(u8);

impl Protocol {
    // https://en.wikipedia.org/wiki/List_of_IP_protocol_numbers
    pub const TCP: Self = Self(0x06);
    pub const UDP: Self = Self(0x11);
    pub fn new(protocol: u8) -> Self {
        Self(protocol)
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
pub struct IpV4HeaderChecksum([u8; 2]);

impl IpV4HeaderChecksum {
    pub fn new() -> Self {
        Self([0, 0])
    }

    pub fn get(self) -> u16 {
        u16::from_be_bytes(self.0)
    }

    pub fn set(&mut self, checksum: u16) {
        self.0 = checksum.to_be_bytes();
    }
}

impl Default for IpV4HeaderChecksum {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    // Example from
    // https://en.wikipedia.org/wiki/IPv4_header_checksum#Calculating_the_IPv4_header_checksum
    #[test]
    fn checksum_generation_works() {
        let mut header = IpV4Header {
            version_ihl: VersionIhl(0x45),
            dscp_ecn: DscpEcn(0),
            len: Length16::new(u16::from_be_bytes([0x00, 0x73])),
            identification: Identification([0x00, 0x00]),
            flags_fragment_offset: FlagsFragmentOffset([0x40, 0x00]),
            ttl: 0x40,
            protocol: Protocol(0x11),
            header_checksum: IpV4HeaderChecksum::new(),
            source_ip: IpV4Address::new(0xC0, 0xA8, 0x00, 0x01),
            destination_ip: IpV4Address::new(0xC0, 0xA8, 0x00, 0xC7),
        };

        header.generate_checksum();
        assert_eq!(header.header_checksum.get(), 0xB861);
    }
}
