// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    primitives::{AlignedReadBuffer, Fields, Primitive, Struct},
    Serializable,
};
use librust::capabilities::CapabilityPtr;

#[derive(Debug, PartialEq, Eq)]
pub enum DeserializeError {
    MalformedOffset,
    BufferTooSmall,
    MismatchedCapabilityType,
    MismatchedId { wanted: u64, found: u64 },
    NotEnoughCapabilities,
    InvalidUtf8,
    InvalidCapabilityProperty,
    UnknownDiscriminantValue,
}

pub struct Deserializer<'a> {
    buffer: &'a [u8],
    capability: Option<CapabilityPtr>,
}

impl<'a> Deserializer<'a> {
    pub fn new(buffer: &'a [u8], capabilities: Option<CapabilityPtr>) -> Self {
        Self { buffer, capability: capabilities }
    }

    #[track_caller]
    pub fn deserialize<T: Deserialize<'a>>(self) -> Result<T, DeserializeError> {
        T::deserialize(
            <T::Primitive<'a> as Primitive>::extract(&mut AlignedReadBuffer::new(self.buffer))?,
            self.capability,
        )
    }
}

pub trait Deserialize<'de>: Serializable + Sized {
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError>;
}

impl<'de> Deserialize<'de> for () {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u8 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i8 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u16 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i16 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u32 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i32 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for u64 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for i64 {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for usize {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for isize {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for &'de str {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de> Deserialize<'de> for alloc::string::String {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(Self::from(primitive))
    }
}

impl<'de, F: for<'a> Fields<'a>> Deserialize<'de> for Struct<'de, F> {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        _: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        Ok(primitive)
    }
}

impl<'de, const LENGTH: usize, D: Deserialize<'de>> Deserialize<'de> for [D; LENGTH] {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        primitive.map(|p| Ok(D::deserialize(p, capabilities).unwrap()))
    }
}

impl<'de, T: Deserialize<'de> + 'de> Deserialize<'de> for alloc::vec::Vec<T> {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        primitive.into_iter().try_fold(alloc::vec::Vec::new(), |mut v, p| {
            v.push(T::deserialize(p?, capabilities)?);
            Ok(v)
        })
    }
}

impl<'de, T: Deserialize<'de> + 'de, E: Deserialize<'de> + 'de> Deserialize<'de> for Result<T, E> {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        match primitive.discriminant()? {
            0 => Ok(Ok(T::deserialize(
                primitive.associated_data::<<T as Serializable>::Primitive<'de>>()?,
                capabilities,
            )?)),
            1 => Ok(Err(E::deserialize(
                primitive.associated_data::<<E as Serializable>::Primitive<'de>>()?,
                capabilities,
            )?)),
            _ => Err(DeserializeError::UnknownDiscriminantValue),
        }
    }
}

impl<'de, T: Deserialize<'de> + 'de> Deserialize<'de> for Option<T> {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        match primitive.discriminant()? {
            0 => Ok(Some(T::deserialize(
                primitive.associated_data::<<T as Serializable>::Primitive<'de>>()?,
                capabilities,
            )?)),
            1 => Ok(None),
            _ => Err(DeserializeError::UnknownDiscriminantValue),
        }
    }
}

impl<'de> Deserialize<'de> for librust::capabilities::Capability {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        capabilities.get(primitive.index).map(|cd| cd.capability).ok_or(DeserializeError::NotEnoughCapabilities)
    }
}

impl<'de> Deserialize<'de> for CapabilityPtr {
    #[inline]
    fn deserialize(
        primitive: <Self as Serializable>::Primitive<'de>,
        capabilities: Option<CapabilityPtr>,
    ) -> Result<Self, DeserializeError> {
        capabilities.get(primitive.index).copied().ok_or(DeserializeError::NotEnoughCapabilities)
    }
}

macro_rules! tuple_deserialize {
    ($($t:ident),+) => {
        tuple_deserialize!(@gen $($t),+);
    };

    (@gen $($t:ident),+) => {
        impl<'de, $($t: Deserialize<'de> + 'de,)+> Deserialize<'de> for ($($t,)+) {
            #[inline]
            #[allow(non_snake_case)]
            fn deserialize(_primitive: <Self as Serializable>::Primitive<'de>, capabilities: Option<CapabilityPtr>,) -> Result<Self, DeserializeError> {
                $(let ($t, _primitive) = _primitive.advance().and_then(|(p, s)| Ok((<$t as Deserialize<'de>>::deserialize(p, capabilities)?, s)))?;)+
                Ok(($($t,)+))
            }
        }

        tuple_deserialize!(@skip1 $($t),+);
    };

    (@gen) => {};

    (@skip1 $head:ident) => {};
    (@skip1 $head:ident, $($t:ident),*) => {
        tuple_deserialize!(@gen $($t),*);
    };

    (@head $head:ident) => { $head };
    (@head $head:ident, $($t:ident),*) => { $head };

    (@tail $head:ident) => {()};
    (@tail $head:ident, $($t:ident),*) => { ($($t,)*) };
}

tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
