// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{ipv4::IpV4Header, BufferTooSmall, Length16};
use alchemy::PackedStruct;

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct UdpHeader {
        pub source_port: Port,
        pub destination_port: Port,
        pub len: Length16,
        pub checksum: UdpChecksum,
    }
}

impl UdpHeader {
    pub fn split_slice_ref(slice: &[u8]) -> Result<(&UdpHeader, &[u8]), BufferTooSmall> {
        if slice.len() < core::mem::size_of::<Self>() {
            return Err(BufferTooSmall);
        }

        let (header, payload) = slice.split_array_ref::<{ core::mem::size_of::<Self>() }>();
        Ok((Self::from_bytes_ref::<{ core::mem::size_of::<Self>() }>(header), payload))
    }

    pub fn split_slice_mut(slice: &mut [u8]) -> Result<(&mut UdpHeader, &mut [u8]), BufferTooSmall> {
        if slice.len() < core::mem::size_of::<Self>() {
            return Err(BufferTooSmall);
        }

        let (header, payload) = slice.split_array_mut::<{ core::mem::size_of::<Self>() }>();
        Ok((Self::from_bytes_mut::<{ core::mem::size_of::<Self>() }>(header), payload))
    }

    pub fn generate_ipv4_checksum(&mut self, _ip_header: &IpV4Header, _data: &[u8]) {
        todo!("generate IPv4 checksum from pseudoheader")
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub struct Port([u8; 2]);
}

impl Port {
    pub fn new(port: u16) -> Self {
        Self(port.to_be_bytes())
    }

    pub fn get(self) -> u16 {
        u16::from_be_bytes(self.0)
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub struct UdpChecksum([u8; 2]);
}

impl UdpChecksum {
    pub fn new() -> Self {
        Self([0; 2])
    }

    pub fn zero(&mut self) {
        self.0 = [0; 2];
    }
}

impl Default for UdpChecksum {
    fn default() -> Self {
        Self::new()
    }
}
