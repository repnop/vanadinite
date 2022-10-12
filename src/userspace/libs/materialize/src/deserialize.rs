// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    primitives::{AlignedReadBuffer, Fields, Primitive, Struct},
    Message, Serializable,
};

pub struct Deserializer<'a> {
    buffer: &'a [u8],
}

impl<'a> Deserializer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }

    pub fn deserialize<T: Deserialize<'a>>(self) -> Result<T, ()> {
        T::deserialize(<T::Primitive<'a> as Primitive>::extract(&mut AlignedReadBuffer::new(self.buffer)).unwrap())
    }
}

pub trait Deserialize<'de>: Serializable + Sized {
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()>;
}

impl<'de> Deserialize<'de> for u8 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i8 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u16 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i16 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u32 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i32 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u64 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i64 {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for usize {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for isize {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for &'de str {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de, F: for<'a> Fields<'a>> Deserialize<'de> for Struct<'de, F> {
    // type Primitive = Self;

    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl<'de, const LENGTH: usize, D: Deserialize<'de>> Deserialize<'de> for [D; LENGTH] {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, ()> {
        primitive.map(|p| Ok(D::deserialize(p).unwrap())).map_err(drop)
    }
}
