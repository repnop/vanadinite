// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{ReservationToken, Serialize, SerializeError, Serializer};
use crate::{
    primitives::{Array, Fields, List, Primitive, Struct},
    sealed,
};

pub trait PrimitiveSerializer<'a>: sealed::Sealed + Sized {
    type Serializer;
    fn construct(serializer: &'a mut Serializer, token: ReservationToken) -> Result<Self::Serializer, SerializeError>;
}

pub struct StructSerializer<'a, F: Fields<'a>> {
    field_token: ReservationToken,
    serializer: &'a mut Serializer,
    _fields: core::marker::PhantomData<fn() -> F>,
}

impl<'a, F: Fields<'a>> StructSerializer<'a, F> {
    pub fn serialize_field<T: Serialize<Primitive<'a> = <F as Fields<'a>>::Head> + ?Sized>(
        self,
        value: &T,
    ) -> Result<StructSerializer<'a, <F as Fields<'a>>::Next>, SerializeError> {
        let Self { field_token, serializer, .. } = self;
        let (token, field_token) = field_token.split(<F as Fields<'_>>::Head::layout())?;

        serializer.serialize_into(token, value)?;

        Ok(StructSerializer { field_token, serializer, _fields: core::marker::PhantomData })
    }
}

impl<'a, F: Fields<'a>> PrimitiveSerializer<'a> for Struct<'a, F> {
    type Serializer = StructSerializer<'a, F>;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        let field_token = serializer.reserve_space(F::layout())?;
        *serializer.integer(&mut token)? = Self::ID;
        *serializer.integer(&mut token)? = field_token.position() as u64;

        Ok(StructSerializer { field_token, serializer, _fields: core::marker::PhantomData })
    }
}

pub struct ArraySerializer<'a, const LENGTH: usize> {
    data_token: ReservationToken,
    serializer: &'a mut Serializer,
}

impl<'a, const LENGTH: usize> ArraySerializer<'a, LENGTH> {
    pub fn serialize_array<T: Serialize>(self, array: &[T; LENGTH]) -> Result<(), SerializeError> {
        let Self { mut data_token, serializer } = self;
        for item in array {
            let (token, rest) = data_token.split(<T::Primitive<'_> as Primitive<'_>>::layout())?;
            data_token = rest;
            serializer.serialize_into(token, item)?;
        }

        Ok(())
    }
}

impl<'a, P: Primitive<'a>, const LENGTH: usize> PrimitiveSerializer<'a> for Array<'a, P, LENGTH> {
    type Serializer = ArraySerializer<'a, LENGTH>;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        let data_token = serializer
            .reserve_space(P::layout().repeat(LENGTH).map_err(|_| SerializeError::NotEnoughSpace)?.0.pad_to_align())?;
        *serializer.integer(&mut token)? = data_token.position();
        *serializer.integer(&mut token)? = LENGTH;

        Ok(ArraySerializer { data_token, serializer })
    }
}

pub struct ListSerializer<'a> {
    token: ReservationToken,
    serializer: &'a mut Serializer,
}

impl<'a> ListSerializer<'a> {
    pub fn serialize_list<T: Serialize>(self, slice: &[T]) -> Result<(), SerializeError> {
        let Self { mut token, serializer } = self;
        let mut data_token = serializer.reserve_space(
            <T::Primitive<'_> as Primitive<'_>>::layout()
                .repeat(slice.len())
                .map_err(|_| SerializeError::NotEnoughSpace)?
                .0
                .pad_to_align(),
        )?;

        *serializer.integer(&mut token)? = data_token.position();
        *serializer.integer(&mut token)? = slice.len();

        for item in slice {
            let (token, rest) = data_token.split(<T::Primitive<'_> as Primitive<'_>>::layout())?;
            data_token = rest;
            serializer.serialize_into(token, item)?;
        }

        Ok(())
    }
}

impl<'a, P: Primitive<'a>> PrimitiveSerializer<'a> for List<'a, P> {
    type Serializer = ListSerializer<'a>;
    fn construct(serializer: &'a mut Serializer, token: ReservationToken) -> Result<Self::Serializer, SerializeError> {
        Ok(ListSerializer { token, serializer })
    }
}

pub struct StringSerializer<'a> {
    token: ReservationToken,
    serializer: &'a mut Serializer,
}

impl<'a> StringSerializer<'a> {
    pub fn serialize_str(self, s: &str) -> Result<(), SerializeError> {
        let Self { mut token, serializer } = self;
        let data_token = serializer.reserve_space(core::alloc::Layout::for_value(s))?;
        *serializer.integer(&mut token)? = data_token.position();
        *serializer.integer(&mut token)? = s.len();
        serializer.buffer_for(data_token)?.copy_from_slice(s.as_bytes());

        Ok(())
    }
}

impl<'a> PrimitiveSerializer<'a> for &'_ str {
    type Serializer = StringSerializer<'a>;
    fn construct(serializer: &'a mut Serializer, token: ReservationToken) -> Result<Self::Serializer, SerializeError> {
        Ok(StringSerializer { token, serializer })
    }
}

impl<'a> PrimitiveSerializer<'a> for () {
    type Serializer = ();
    fn construct(_: &mut Serializer, _: ReservationToken) -> Result<Self::Serializer, SerializeError> {
        Ok(())
    }
}

impl<'a> PrimitiveSerializer<'a> for u8 {
    type Serializer = &'a mut u8;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for i8 {
    type Serializer = &'a mut i8;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for u16 {
    type Serializer = &'a mut u16;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for i16 {
    type Serializer = &'a mut i16;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for u32 {
    type Serializer = &'a mut u32;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for i32 {
    type Serializer = &'a mut i32;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for u64 {
    type Serializer = &'a mut u64;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for i64 {
    type Serializer = &'a mut i64;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for usize {
    type Serializer = &'a mut usize;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}

impl<'a> PrimitiveSerializer<'a> for isize {
    type Serializer = &'a mut isize;
    fn construct(
        serializer: &'a mut Serializer,
        mut token: ReservationToken,
    ) -> Result<Self::Serializer, SerializeError> {
        serializer.integer(&mut token)
    }
}
