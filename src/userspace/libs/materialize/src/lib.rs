// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![allow(incomplete_features)]
#![feature(generic_const_exprs, strict_provenance)]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod deserialize;
mod hash;
pub mod primitives;
mod serialize;
pub mod writer;

use primitives::Primitive;

const MINIMUM_ALIGNMENT: usize = core::mem::align_of::<u64>();

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
