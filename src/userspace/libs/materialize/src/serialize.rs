// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod serializers;

use crate::{
    buffer::AlignedHeapBuffer,
    primitives::{Integer, Primitive},
    Serializable,
};
use alloc::alloc::Layout;
use librust::capabilities::Capability;
use serializers::PrimitiveSerializer;

#[derive(Debug)]
pub enum SerializeError {
    AllocationError,
    NotEnoughSpace,
    MisalignedPosition,
    TooManyCapabilities,
}

impl From<core::alloc::AllocError> for SerializeError {
    fn from(_: core::alloc::AllocError) -> Self {
        Self::AllocationError
    }
}

pub struct ReservationToken {
    position: usize,
    length: usize,
}

impl ReservationToken {
    #[inline(always)]
    pub(crate) fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub(crate) fn align(&mut self, layout: Layout) -> Result<(), SerializeError> {
        let align = layout.align();
        if self.position % align == 0 {
            return Ok(());
        }

        let padding = align - (self.position % align);

        if self.length - padding < layout.size() {
            return Err(SerializeError::NotEnoughSpace);
        }

        self.position += padding;
        self.length -= padding;

        Ok(())
    }

    pub(crate) fn split(mut self, layout: Layout) -> Result<(Self, Self), SerializeError> {
        self.align(layout)?;
        let second = Self { position: self.position + layout.size(), length: self.length - layout.size() };
        self.length = layout.size();

        Ok((self, second))
    }
}

pub struct Serializer {
    buffer: AlignedHeapBuffer,
    capability: Option<Capability>,
}

impl Serializer {
    pub fn new() -> Self {
        Self { buffer: AlignedHeapBuffer::new(), capability: None }
    }

    pub fn into_buffer(self) -> AlignedHeapBuffer {
        self.buffer
    }

    pub fn into_parts(self) -> (AlignedHeapBuffer, Option<Capability>) {
        (self.buffer, self.capability)
    }

    pub fn serialize<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerializeError> {
        let token = self.reserve_space(<T::Primitive<'_> as Primitive<'_>>::layout())?;
        self.serialize_into(token, value)
    }

    pub(crate) fn serialize_into<T: Serialize + ?Sized>(
        &mut self,
        token: ReservationToken,
        value: &T,
    ) -> Result<(), SerializeError> {
        value.serialize(<T::Primitive<'_> as PrimitiveSerializer>::construct(self, token)?)
    }

    pub(crate) fn reserve_space(&mut self, layout: Layout) -> Result<ReservationToken, SerializeError> {
        self.align_to(layout.align())?;

        let current_len = self.buffer.len();
        self.buffer.resize(current_len + layout.size(), 0)?;

        Ok(ReservationToken { position: current_len, length: layout.size() })
    }

    pub(crate) fn buffer_for(&mut self, token: ReservationToken) -> Result<&mut [u8], SerializeError> {
        self.buffer.get_mut(token.position..token.position + token.length).ok_or(SerializeError::NotEnoughSpace)
    }

    pub(crate) fn integer<I: Integer>(&mut self, token: &mut ReservationToken) -> Result<&mut I, SerializeError> {
        let tkn = core::mem::replace(token, ReservationToken { position: 0, length: 0 });
        let (tkn, rest) = tkn.split(core::alloc::Layout::new::<I>())?;
        *token = rest;
        Ok(unsafe { &mut *self.buffer.as_mut_ptr().add(tkn.position).cast() })
    }

    // #[track_caller]
    // pub(crate) fn write_bytes(&mut self, token: &mut ReservationToken, bytes: &[u8]) -> Result<(), SerializeError> {
    //     if bytes.len() > token.length {
    //         return Err(SerializeError::NotEnoughSpace);
    //     }

    //     self.buffer[token.position..][..bytes.len()].copy_from_slice(bytes);
    //     *token = ReservationToken { position: token.position + bytes.len(), length: token.length - bytes.len() };
    //     Ok(())
    // }

    fn align_to(&mut self, align: usize) -> Result<(), SerializeError> {
        let current_len = self.buffer.len();

        if current_len % align == 0 {
            return Ok(());
        }

        let padding = align - (current_len % align);
        self.buffer.resize(current_len + padding, 0)?;

        Ok(())
    }
}

impl Default for Serializer {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Serialize: Serializable {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError>;
}

impl Serialize for () {
    fn serialize<'a>(
        &self,
        _: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(())
    }
}

impl Serialize for u8 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for i8 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for u16 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for i16 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for u32 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for i32 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for u64 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for i64 {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for usize {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for isize {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        Ok(*serializer = *self)
    }
}

impl Serialize for &'_ str {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        serializer.serialize_str(self)
    }
}

impl Serialize for alloc::string::String {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        serializer.serialize_str(self)
    }
}

impl<T: Serialize, const LENGTH: usize> Serialize for [T; LENGTH] {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        serializer.serialize_array(self)
    }
}

impl<T: Serialize> Serialize for alloc::vec::Vec<T> {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        serializer.serialize_list(self)
    }
}

impl<T: Serialize> Serialize for [T] {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        serializer.serialize_list(self)
    }
}

impl<T: Serialize, E: Serialize> Serialize for Result<T, E> {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        match self {
            Ok(t) => serializer.serialize_variant(&0u32, t),
            Err(e) => serializer.serialize_variant(&1u32, e),
        }
    }
}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        match self {
            Some(t) => serializer.serialize_variant(&0u32, t),
            None => serializer.serialize_variant(&1u32, &()),
        }
    }
}

impl Serialize for librust::capabilities::Capability {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        serializer.serialize_capability(*self)
    }
}

impl<T: Serialize + ?Sized> Serialize for &'_ T {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), SerializeError> {
        <T as Serialize>::serialize(*self, serializer)
    }
}

macro_rules! tuple_serialize {
    ($($t:ident),+) => {
        tuple_serialize!(@gen $($t),+);
    };

    (@gen $($t:ident),+) => {
        impl<$($t: Serialize,)+> Serialize for ($($t,)+) {
            fn serialize<'a>(
                &self,
                _serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
            ) -> Result<(), SerializeError> {
                $(let _serializer = _serializer.serialize_field(&&self.${index()})?; ${ignore($t)})+

                Ok(())
            }
        }

        tuple_serialize!(@skip1 $($t),+);
    };

    (@gen) => {};

    (@skip1 $head:ident) => {};
    (@skip1 $head:ident, $($t:ident),*) => {
        tuple_serialize!(@gen $($t),*);
    };

    (@head $head:ident) => { $head };
    (@head $head:ident, $($t:ident),*) => { $head };

    (@tail $head:ident) => {()};
    (@tail $head:ident, $($t:ident),*) => { ($($t,)*) };
}

tuple_serialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        deserialize::{Deserialize, Deserializer},
        primitives::{AlignedReadBuffer, Array, List, Struct},
        DeserializeError, Serializable, Serialize,
    };
    use materialize_derive::Deserialize;

    #[test]
    fn roundtrip_struct() {
        struct MyCoolStruct;

        impl Serializable for MyCoolStruct {
            type Primitive<'a> = Struct<'a, (u64, u32, u8, &'a str)>;
        }

        impl Serialize for MyCoolStruct {
            fn serialize<'a>(
                &self,
                serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
            ) -> Result<(), SerializeError> {
                let serializer = serializer.serialize_field(&0xDEADF00DBEEFBABEu64)?;
                let serializer = serializer.serialize_field(&0xC0BB0000u32)?;
                let serializer = serializer.serialize_field(&0xF0u8)?;
                serializer.serialize_field("TESTyeet")?;

                Ok(())
            }
        }

        impl<'de> Deserialize<'de> for MyCoolStruct {
            // type Primitive = Struct<'de, (u64, u32, u8, &'de str)>;

            fn deserialize(strukt: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
                assert_eq!(strukt.field(), Ok(0xDEADF00DBEEFBABE));
                assert_eq!(strukt.next().field(), Ok(0xC0BB0000));
                assert_eq!(strukt.next().next().field(), Ok(0xF0));
                assert_eq!(strukt.next().next().next().field(), Ok("TESTyeet"));
                Ok(Self)
            }
        }

        let mut serializer = Serializer::new();
        serializer.serialize(&MyCoolStruct).unwrap();
        let mut deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        deserializer.deserialize::<MyCoolStruct>().unwrap();
    }

    #[test]
    fn complex_struct() {
        #[derive(Debug, PartialEq)]
        struct ComplexStruct {
            frabs: [LittleStruct; 5],
        }

        impl Serializable for ComplexStruct {
            type Primitive<'a> = Struct<'a, (<[LittleStruct; 5] as Serializable>::Primitive<'a>,)>;
        }

        impl Serialize for ComplexStruct {
            fn serialize<'a>(
                &self,
                serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
            ) -> Result<(), SerializeError> {
                serializer.serialize_field(&self.frabs)?;
                Ok(())
            }
        }

        impl<'de> Deserialize<'de> for ComplexStruct {
            fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
                Ok(Self { frabs: <_>::deserialize(primitive.field()?)? })
            }
        }

        #[derive(Debug, PartialEq)]
        struct LittleStruct {
            frab: u32,
        }

        impl Serializable for LittleStruct {
            type Primitive<'a> = Struct<'a, (u32,)>;
        }

        impl Serialize for LittleStruct {
            fn serialize<'a>(
                &self,
                serializer: <Self::Primitive<'a> as PrimitiveSerializer<'a>>::Serializer,
            ) -> Result<(), SerializeError> {
                serializer.serialize_field(&self.frab)?;
                Ok(())
            }
        }

        impl<'de> Deserialize<'de> for LittleStruct {
            fn deserialize(primitive: <Self as Serializable>::Primitive<'de>) -> Result<Self, DeserializeError> {
                Ok(Self { frab: primitive.field()? })
            }
        }

        let mut serializer = Serializer::new();
        let strukt = ComplexStruct {
            frabs: [
                LittleStruct { frab: 0 },
                LittleStruct { frab: 1 },
                LittleStruct { frab: 0x5555 },
                LittleStruct { frab: 0xAAAAAA },
                LittleStruct { frab: u32::MAX },
            ],
        };

        serializer.serialize(&strukt).unwrap();
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<ComplexStruct>(), Ok(strukt));
    }

    #[test]
    fn vec() {
        #[derive(Debug, PartialEq, Serializable, Deserialize, Serialize)]
        #[materialize(reexport_path = "crate")]
        struct Padding(u32, u8);

        let v = std::vec![Padding(0xAA55AA55, 0xFF), Padding(0x22DD22DD, 0x01)];
        let mut serializer = Serializer::new();
        serializer.serialize(&v).unwrap();
        pretty_print_buffer(&serializer.buffer);
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<std::vec::Vec<Padding>>(), Ok(v));
    }

    fn pretty_print_buffer(b: &[u8]) {
        for (i, chunk) in b.chunks(8).enumerate() {
            std::print!("{:<02x}:    ", i * 8);
            for b in chunk {
                std::print!("{b:<02x} ");
            }
            std::println!();
        }
    }

    #[test]
    fn enoom() {
        #[derive(Debug, PartialEq, Serializable, Deserialize, Serialize)]
        #[materialize(reexport_path = "crate")]
        enum Fraz {
            Qux,
            Baz,
            Yeet = 6,
        }

        let mut serializer = Serializer::new();
        serializer.serialize(&Fraz::Qux);
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<Fraz>(), Ok(Fraz::Qux));

        let mut serializer = Serializer::new();
        serializer.serialize(&Fraz::Baz);
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<Fraz>(), Ok(Fraz::Baz));

        let mut serializer = Serializer::new();
        serializer.serialize(&Fraz::Yeet);
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<Fraz>(), Ok(Fraz::Yeet));

        #[derive(Debug, PartialEq, Serializable, Deserialize, Serialize)]
        #[materialize(reexport_path = "crate")]
        enum Fraz2 {
            Qux(std::string::String),
            Baz { my_special_int: u32 },
            Yeet(std::vec::Vec<(u8, isize)>),
        }

        let mut serializer = Serializer::new();
        serializer.serialize(&Fraz2::Qux(std::string::String::from("pindakaas")));
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<Fraz2>(), Ok(Fraz2::Qux(std::string::String::from("pindakaas"))));

        let mut serializer = Serializer::new();
        serializer.serialize(&Fraz2::Baz { my_special_int: 0x55AA55AA });
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<Fraz2>(), Ok(Fraz2::Baz { my_special_int: 0x55AA55AA }));

        let mut serializer = Serializer::new();
        serializer.serialize(&Fraz2::Yeet(std::vec![(1, -1), (2, -2), (3, -3)]));
        let deserializer = Deserializer::new(&serializer.buffer[..], &[]);
        assert_eq!(deserializer.deserialize::<Fraz2>(), Ok(Fraz2::Yeet(std::vec![(1, -1), (2, -2), (3, -3)])));
    }
}
