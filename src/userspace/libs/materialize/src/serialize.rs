// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    primitives::{Fields, Primitive},
    writer::MessageWriter,
};

#[derive(Debug)]
pub enum SerializeError {
    NotEnoughSpace,
}

pub struct ReservationToken {
    position: usize,
    length: usize,
}

impl ReservationToken {
    pub(crate) fn position(&self) -> usize {
        self.position
    }
}

pub struct Serializer {
    buffer: alloc::vec::Vec<u8>,
}

impl Serializer {
    pub fn new() -> Self {
        Self { buffer: alloc::vec::Vec::new() }
    }

    pub fn serialize<P: Primitive>(&mut self, input: P::Input<'_>) -> Result<(), SerializeError> {
        let mut token = self.reserve_space(P::layout())?;
        P::encode(input, self, &mut token)
    }

    pub(crate) fn reserve_space(&mut self, layout: alloc::alloc::Layout) -> Result<ReservationToken, SerializeError> {
        self.align_to(layout.align())?;

        let current_len = self.buffer.len();
        self.buffer.resize(current_len + layout.size(), 0);

        Ok(ReservationToken { position: current_len, length: layout.size() })
    }

    #[track_caller]
    pub(crate) fn write_bytes(&mut self, token: &mut ReservationToken, bytes: &[u8]) -> Result<(), SerializeError> {
        if bytes.len() > token.length {
            return Err(SerializeError::NotEnoughSpace);
        }

        self.buffer[token.position..][..bytes.len()].copy_from_slice(bytes);
        *token = ReservationToken { position: token.position + bytes.len(), length: token.length - bytes.len() };
        Ok(())
    }

    fn align_to(&mut self, align: usize) -> Result<(), SerializeError> {
        let current_len = self.buffer.len();

        if current_len % align == 0 {
            return Ok(());
        }

        let padding = align - (current_len % align);
        self.buffer.resize(current_len + padding, 0);

        Ok(())
    }
}

pub trait Serialize {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError>;
}

impl Serialize for () {
    fn serialize(&self, _: &mut Serializer) -> Result<(), SerializeError> {
        Ok(())
    }
}

impl Serialize for u8 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<u8>(*self)
    }
}

impl Serialize for i8 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<i8>(*self)
    }
}

impl Serialize for u16 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<u16>(*self)
    }
}

impl Serialize for i16 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<i16>(*self)
    }
}

impl Serialize for u32 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<u32>(*self)
    }
}

impl Serialize for i32 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<i32>(*self)
    }
}

impl Serialize for u64 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<u64>(*self)
    }
}

impl Serialize for i64 {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<i64>(*self)
    }
}

impl Serialize for usize {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<usize>(*self)
    }
}

impl Serialize for isize {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<isize>(*self)
    }
}

impl Serialize for str {
    fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
        writer.serialize::<&str>(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        deserialize::Deserialize,
        primitives::{AlignedReadBuffer, Struct},
    };

    #[test]
    fn struct_extract() {
        struct MyCoolStruct;

        impl Serialize for MyCoolStruct {
            fn serialize(&self, writer: &mut Serializer) -> Result<(), SerializeError> {
                writer.serialize::<Struct<(u64, u32, u8, &str)>>((0xDEADF00DBEEFBABEu64, 0xC0BB0000, 0xF0, "TESTyeet"))
            }
        }

        impl Deserialize for MyCoolStruct {
            type Primitive<'a> = Struct<'a, (u64, u32, u8, &'a str)>;

            fn deserialize(strukt: Self::Primitive<'_>) -> Result<Self, ()> {
                assert_eq!(strukt.field(), Ok(0xDEADF00DBEEFBABE));
                assert_eq!(strukt.next().field(), Ok(0xC0BB0000));
                assert_eq!(strukt.next().next().field(), Ok(0xF0));
                assert_eq!(strukt.next().next().next().field(), Ok("TESTyeet"));
                Ok(Self)
            }
        }

        let mut serializer = Serializer::new();
        MyCoolStruct.serialize(&mut serializer).unwrap();
        MyCoolStruct::deserialize(AlignedReadBuffer::new(&serializer.buffer[..])).unwrap();
        panic!("{:x?}", serializer.buffer);
    }
}
