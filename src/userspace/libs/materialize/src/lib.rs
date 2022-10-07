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

pub trait Serializable {
    type Primitive<'a>: primitives::Primitive;
}

impl Serializable for u8 {
    type Primitive<'a> = u8;
}

impl Serializable for i8 {
    type Primitive<'a> = i8;
}

impl Serializable for u16 {
    type Primitive<'a> = u16;
}

impl Serializable for i16 {
    type Primitive<'a> = i16;
}

impl Serializable for u32 {
    type Primitive<'a> = u32;
}

impl Serializable for i32 {
    type Primitive<'a> = i32;
}

impl Serializable for u64 {
    type Primitive<'a> = u64;
}

impl Serializable for i64 {
    type Primitive<'a> = i64;
}

impl Serializable for usize {
    type Primitive<'a> = usize;
}

impl Serializable for isize {
    type Primitive<'a> = isize;
}

impl Serializable for &'_ str {
    type Primitive<'a> = &'a str;
}
