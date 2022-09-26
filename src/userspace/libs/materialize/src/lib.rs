// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![feature(strict_provenance)]

extern crate alloc;

pub mod primitives;

use primitives::Primitive;

const MINIMUM_ALIGNMENT: usize = core::mem::align_of::<usize>();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Underaligned;

pub struct Message<'a> {
    buffer: &'a [u8],
}

impl<'a> Message<'a> {
    pub fn new(buffer: &'a [u8]) -> Result<Self, Underaligned> {
        match buffer.as_ptr().addr() % MINIMUM_ALIGNMENT {
            0 => Ok(Self { buffer }),
            _ => Err(Underaligned),
        }
    }

    pub fn read<P: Primitive + 'a>(&self) -> Result<P, ()> {
        todo!()
    }
}
