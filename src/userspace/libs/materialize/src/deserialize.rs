// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    primitives::{AlignedReadBuffer, Fields, List, Primitive, Struct},
    Serializable,
};

#[derive(Debug, PartialEq, Eq)]
pub enum DeserializeError {
    MalformedOffset,
    BufferTooSmall,
    MismatchedId { wanted: u64, found: u64 },
    InvalidUtf8,
}

pub struct Deserializer<'a> {
    buffer: &'a [u8],
}

impl<'a> Deserializer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }

    pub fn deserialize<T: Deserialize<'a>>(self) -> Result<T, DeserializeError> {
        T::deserialize(<T::Primitive<'a> as Primitive>::extract(&mut AlignedReadBuffer::new(self.buffer)).unwrap())
    }
}

pub trait Deserialize<'de>: Serializable + Sized {
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError>;
}

impl<'de> Deserialize<'de> for u8 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i8 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u16 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i16 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u32 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i32 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u64 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i64 {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for usize {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for isize {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for &'de str {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for alloc::string::String {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(Self::from(primitive))
    }
}

impl<'de, F: for<'a> Fields<'a>> Deserialize<'de> for Struct<'de, F> {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de, const LENGTH: usize, D: Deserialize<'de>> Deserialize<'de> for [D; LENGTH] {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        primitive.map(|p| Ok(D::deserialize(p).unwrap()))
    }
}

impl<'de, T: Deserialize<'de> + 'de> Deserialize<'de> for alloc::vec::Vec<T> {
    #[inline]
    fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
        primitive.into_iter().try_fold(alloc::vec::Vec::new(), |mut v, p| {
            v.push(T::deserialize(p?)?);
            Ok(v)
        })
    }
}
