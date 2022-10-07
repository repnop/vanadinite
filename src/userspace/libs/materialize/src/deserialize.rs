// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    primitives::{AlignedReadBuffer, Fields, Primitive, Struct},
    Message,
};

pub struct Deserializer<'a> {
    buffer: &'a [u8],
}

impl<'a> Deserializer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }

    pub fn deserialize<T: Deserialize>(self) -> Result<T, ()> {
        T::deserialize(<T::Primitive<'_> as Primitive>::extract(&mut AlignedReadBuffer::new(self.buffer)).unwrap())
    }
}

pub trait Deserialize: Sized {
    type Primitive<'a>: Primitive;

    fn deserialize<'a>(primitive: <Self::Primitive<'a> as Primitive>::Output<'a>) -> Result<Self, ()>;
}

impl Deserialize for u8 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for i8 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for u16 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for i16 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for u32 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for i32 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for u64 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for i64 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for usize {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for isize {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'a> Deserialize for &'a str {
    type Primitive<'b> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<F: Fields> Deserialize for Struct<'_, F> {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: <Self::Primitive<'_> as Primitive>::Output<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}
