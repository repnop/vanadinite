// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![allow(incomplete_features, clippy::unit_arg)]
#![feature(
    alloc_layout_extra,
    allocator_api,
    array_methods,
    array_try_map,
    const_slice_from_raw_parts_mut,
    generic_const_exprs,
    macro_metavar_expr,
    slice_ptr_get,
    strict_provenance
)]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod buffer;
mod deserialize;
mod hash;
pub mod primitives;
mod serialize;
pub mod writer;

use primitives::Primitive;

const MINIMUM_ALIGNMENT: usize = core::mem::align_of::<u64>();

mod sealed {
    pub trait Sealed {}
}

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

    pub fn read<P: Primitive<'a> + 'a>(&self) -> Result<P, ()> {
        todo!()
    }
}

pub trait Serializable {
    type Primitive<'a>: primitives::Primitive<'a>;
}

impl Serializable for () {
    type Primitive<'a> = ();
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

impl Serializable for str {
    type Primitive<'a> = &'a str;
}

impl Serializable for &'_ str {
    type Primitive<'a> = &'a str;
}

impl<F: for<'a> primitives::Fields<'a>> Serializable for primitives::Struct<'_, F> {
    type Primitive<'b> = primitives::Struct<'b, F>;
}

impl<const LENGTH: usize, S: Serializable> Serializable for [S; LENGTH] {
    type Primitive<'a> = primitives::Array<'a, S::Primitive<'a>, LENGTH>;
}
