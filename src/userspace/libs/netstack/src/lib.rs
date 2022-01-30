// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![allow(incomplete_features)]
#![feature(generic_arg_infer, generic_const_exprs, split_array, array_chunks)]

pub mod arp;
pub mod ethernet;
pub mod ipv4;
pub mod udp;

alchemy::derive! {
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct MacAddress([u8; 6]);
}

impl MacAddress {
    pub const BROADCAST: Self = Self([0xFF; 6]);

    pub fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    pub fn bytes(self) -> [u8; 6] {
        self.0
    }
}

impl core::fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:0>2X}:{:0>2X}:{:0>2X}:{:0>2X}:{:0>2X}:{:0>2X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BufferTooSmall;

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub struct Length16([u8; 2]);
}

impl Length16 {
    pub fn new(len: u16) -> Self {
        Self(len.to_be_bytes())
    }

    pub fn get(self) -> u16 {
        u16::from_be_bytes(self.0)
    }
}
