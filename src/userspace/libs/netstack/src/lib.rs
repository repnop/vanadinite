// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![feature(generic_arg_infer, split_array, array_chunks)]

pub mod ethernet;
pub mod ipv4;
pub mod udp;

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
