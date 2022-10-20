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
pub mod deserialize;
mod hash;
pub mod primitives;
pub mod serialize;
pub mod writer;

pub use deserialize::{Deserialize, DeserializeError, Deserializer};
pub use materialize_derive::*;
use primitives::Primitive;
pub use serialize::{Serialize, SerializeError, Serializer};

const MINIMUM_ALIGNMENT: usize = core::mem::align_of::<u64>();

mod sealed {
    pub trait Sealed {}
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

impl Serializable for alloc::string::String {
    type Primitive<'a> = &'a str;
}

impl<T: Serializable> Serializable for alloc::vec::Vec<T> {
    type Primitive<'a> = primitives::List<'a, T::Primitive<'a>>;
}

impl<F: for<'a> primitives::Fields<'a>> Serializable for primitives::Struct<'_, F> {
    type Primitive<'b> = primitives::Struct<'b, F>;
}

impl<const LENGTH: usize, S: Serializable> Serializable for [S; LENGTH] {
    type Primitive<'a> = primitives::Array<'a, S::Primitive<'a>, LENGTH>;
}

impl<T: Serializable> Serializable for &'_ T {
    type Primitive<'a> = <T as Serializable>::Primitive<'a>;
}

macro_rules! tuple_serializable {
    ($($t:ident),+) => {
        tuple_serializable!(@gen $($t),+);
    };

    (@gen $($t:ident),+) => {
        impl<$($t: Serializable,)+> Serializable for ($($t,)+) {
            type Primitive<'a> = primitives::Struct<'a, ($(<$t as Serializable>::Primitive<'a>,)+)>;
        }

        tuple_serializable!(@skip1 $($t),+);
    };

    (@gen) => {};

    (@skip1 $head:ident) => {};
    (@skip1 $head:ident, $($t:ident),*) => {
        tuple_serializable!(@gen $($t),*);
    };

    (@head $head:ident) => { $head };
    (@head $head:ident, $($t:ident),*) => { $head };

    (@tail $head:ident) => {()};
    (@tail $head:ident, $($t:ident),*) => { ($($t,)*) };
}

tuple_serializable!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
