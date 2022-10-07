// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod fields;

use crate::{
    hash::FxHasher,
    serialize::{ReservationToken, SerializeError, Serializer},
};
use core::convert::TryFrom;

mod sealed {
    pub trait Sealed {}
}

unsafe trait Integer: Sized + Copy {}
unsafe impl Integer for u8 {}
unsafe impl Integer for i8 {}
unsafe impl Integer for u16 {}
unsafe impl Integer for i16 {}
unsafe impl Integer for u32 {}
unsafe impl Integer for i32 {}
unsafe impl Integer for u64 {}
unsafe impl Integer for i64 {}
unsafe impl Integer for usize {}
unsafe impl Integer for isize {}
unsafe impl<const N: usize, I: Integer> Integer for [I; N] {}

#[derive(Clone)]
pub struct AlignedReadBuffer<'a> {
    buffer: &'a [u8],
    position: usize,
}

impl<'a> AlignedReadBuffer<'a> {
    pub(crate) fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, position: 0 }
    }

    #[inline]
    fn read<I: Integer>(&mut self) -> Result<I, ExtractionError> {
        let buffer = self.buffer.get(self.position..).ok_or(ExtractionError::BufferTooSmall)?;
        let slice = unsafe { buffer.align_to::<I>().1 };
        match slice {
            [] => Err(ExtractionError::BufferTooSmall),
            [value, ..] => {
                self.position += core::mem::size_of::<I>();
                Ok(*value)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtractionError {
    MalformedOffset,
    BufferTooSmall,
    MismatchedId { wanted: u64, found: u64 },
    InvalidUtf8,
}

pub trait Primitive: sealed::Sealed {
    const ID: u64;
    type Input<'a>;
    type Output<'a>;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError>;
    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError>;
    fn layout() -> core::alloc::Layout;
}

impl Primitive for () {
    const ID: u64 = 0xadc4eb49d6e3a43c;
    type Input<'a> = ();
    type Output<'a> = ();

    fn encode(_: Self::Input<'_>, _: &mut Serializer, _: &mut ReservationToken) -> Result<(), SerializeError> {
        Ok(())
    }

    fn extract<'a>(_: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        Ok(())
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<()>()
    }
}

pub struct Struct<'a, F: Fields> {
    fields: core::marker::PhantomData<fn() -> F>,
    buffer: AlignedReadBuffer<'a>,
}

impl<'a, F: Fields> Struct<'a, F> {
    pub const STRUCT_BASE_ID: u64 = 0x8877eea67b715863;

    #[inline]
    pub fn field(&self) -> Result<<F::Head as Primitive>::Output<'a>, ExtractionError> {
        <F::Head as Primitive>::extract(&mut self.buffer.clone())
    }

    #[inline]
    pub fn next(&self) -> Struct<'a, <F as Fields>::Next> {
        let mut buffer = self.buffer.clone();
        buffer.position += <<F as Fields>::Head as Primitive>::layout().size();
        Struct { buffer, fields: core::marker::PhantomData }
    }

    #[inline]
    pub fn advance(
        self,
    ) -> Result<(<F::Head as Primitive>::Output<'a>, Struct<'a, <F as Fields>::Next>), ExtractionError> {
        Ok((self.field()?, self.next()))
    }
}

impl<F: Fields> sealed::Sealed for Struct<'_, F> {}
impl<F: Fields> Primitive for Struct<'_, F> {
    const ID: u64 = FxHasher::new().hash(Self::STRUCT_BASE_ID).hash(<<F as Fields>::Head as Primitive>::ID).finish();
    type Input<'a> = F::Input<'a>;
    type Output<'a> = Struct<'a, F>;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        let mut field_token = serializer.reserve_space(F::layout())?;
        serializer.write_bytes(token, &Self::ID.to_ne_bytes()[..])?;
        serializer.write_bytes(token, &(field_token.position() as u64).to_ne_bytes()[..])?;
        F::encode(input, serializer, &mut field_token)
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        let [id, position] = buffer.read::<[u64; 2]>()?;
        let position = usize::try_from(position).map_err(|_| ExtractionError::MalformedOffset)?;

        if id != Self::ID {
            return Err(ExtractionError::MismatchedId { wanted: Self::ID, found: id });
        } else if buffer.buffer.get(position..position + F::layout().size()).is_none() {
            return Err(ExtractionError::MalformedOffset);
        }

        Ok(Struct { buffer: AlignedReadBuffer { buffer: buffer.buffer, position }, fields: core::marker::PhantomData })
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<[u64; 2]>()
    }
}

impl sealed::Sealed for &'_ str {}
impl Primitive for &'_ str {
    const ID: u64 = 0x94a845be7716094d;
    type Input<'a> = &'a str;
    type Output<'a> = &'a str;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        match input.len() {
            0 => serializer.write_bytes(token, &[0; 16]).map(drop),
            len => {
                let mut data_token = serializer.reserve_space(core::alloc::Layout::for_value(input))?;
                serializer.write_bytes(token, &data_token.position().to_ne_bytes()[..])?;
                serializer.write_bytes(token, &len.to_ne_bytes()[..])?;
                serializer.write_bytes(&mut data_token, input.as_bytes())?;
                Ok(())
            }
        }
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;
        let buffer = buffer.buffer.get(position..position + length).ok_or(ExtractionError::MalformedOffset)?;

        if position == 0 {
            return Ok("");
        }

        core::str::from_utf8(buffer).map_err(|_| ExtractionError::InvalidUtf8)
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<[usize; 2]>()
    }
}

pub struct Array<'a, P: Primitive, const LENGTH: usize> {
    fields: core::marker::PhantomData<fn() -> P>,
    buffer: AlignedReadBuffer<'a>,
}

impl<'a, P: Primitive, const LENGTH: usize> Array<'a, P, LENGTH> {
    pub const ARRAY_BASE_ID: u64 = 0xf13a444fbc5162d0;

    #[inline]
    pub fn field(&self) -> Result<<P as Primitive>::Output<'a>, ExtractionError> {
        <P as Primitive>::extract(&mut self.buffer.clone())
    }

    #[inline]
    pub fn skip<const N: usize>(&self) -> Array<'a, P, { LENGTH - N }> {
        let mut buffer = self.buffer.clone();
        buffer.position += <P as Primitive>::layout().size() * N;
        Array { buffer, fields: core::marker::PhantomData }
    }

    #[inline]
    pub fn pop_front(self) -> Result<(<P as Primitive>::Output<'a>, Array<'a, P, { LENGTH - 1 }>), ExtractionError> {
        Ok((self.field()?, self.skip::<1>()))
    }
}

impl<const LENGTH: usize, P: Primitive> sealed::Sealed for Array<'_, P, LENGTH> {}
impl<const LENGTH: usize, P: Primitive> Primitive for Array<'_, P, LENGTH> {
    const ID: u64 = FxHasher::new().hash(0xf13a444fbc5162d0).hash(P::ID).hash(LENGTH as u64).finish();
    type Input<'a> = [P::Input<'a>; LENGTH];
    type Output<'a> = Array<'a, P, LENGTH>;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        for input in input {
            P::encode(input, serializer, token)?;
        }

        Ok(())
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;

        if buffer.buffer.get(position..position + (P::layout().size() * length)).is_none() {
            return Err(ExtractionError::MalformedOffset);
        }

        Ok(Array { buffer: AlignedReadBuffer { buffer: buffer.buffer, position }, fields: core::marker::PhantomData })
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<[usize; 2]>()
    }
}

impl sealed::Sealed for u8 {}
impl Primitive for u8 {
    const ID: u64 = 0xd4d1d74109db7e0;
    type Input<'a> = u8;
    type Output<'a> = u8;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &[input])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for i8 {}
impl Primitive for i8 {
    const ID: u64 = 0x85316d595ee12d8e;
    type Input<'a> = i8;
    type Output<'a> = i8;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &[input as u8])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for u16 {}
impl Primitive for u16 {
    const ID: u64 = 0x182ca144e057ded8;
    type Input<'a> = u16;
    type Output<'a> = u16;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for i16 {}
impl Primitive for i16 {
    const ID: u64 = 0x8339ca9fef21af4;
    type Input<'a> = i16;
    type Output<'a> = i16;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for u32 {}
impl Primitive for u32 {
    const ID: u64 = 0xb330c6b1bc925fe3;
    type Input<'a> = u32;
    type Output<'a> = u32;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for i32 {}
impl Primitive for i32 {
    const ID: u64 = 0xa7618d5014e22dcd;
    type Input<'a> = i32;
    type Output<'a> = i32;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for u64 {}
impl Primitive for u64 {
    const ID: u64 = 0x46f3003d096708b8;
    type Input<'a> = u64;
    type Output<'a> = u64;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for i64 {}
impl Primitive for i64 {
    const ID: u64 = 0xf892cc40250d39f7;
    type Input<'a> = i64;
    type Output<'a> = i64;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for usize {}
impl Primitive for usize {
    const ID: u64 = 0x191f7db76a9b101d;
    type Input<'a> = usize;
    type Output<'a> = usize;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

impl sealed::Sealed for isize {}
impl Primitive for isize {
    const ID: u64 = 0xe14dbb5b71ba5adc;
    type Input<'a> = isize;
    type Output<'a> = isize;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError> {
        serializer.write_bytes(token, &input.to_ne_bytes()[..])
    }

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<Self>()
    }
}

pub trait Fields: Sized + sealed::Sealed {
    const ID: u64;
    type Head: Primitive;
    type Next: Fields;
    type Input<'a>;

    fn encode(
        input: Self::Input<'_>,
        serializer: &mut Serializer,
        token: &mut ReservationToken,
    ) -> Result<(), SerializeError>;

    fn layout() -> core::alloc::Layout {
        <Self::Head as Primitive>::layout().extend(<Self::Next as Fields>::layout()).unwrap().0.pad_to_align()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn struct_extract() {
        type TestStruct<'a> = Struct<'a, (u64, u32, u8, &'a str)>;
        let buffer =
            [<TestStruct as Primitive>::ID, 16, 0xDEADF00DBEEFBABEu64, 0x000000F0C0BB0000, 48, 8, 0x7465657954534554];
        let mut buf =
            AlignedReadBuffer::new(unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast(), buffer.len() * 8) });
        let strukt = TestStruct::extract(&mut buf).unwrap();
        assert_eq!(strukt.field(), Ok(0xDEADF00DBEEFBABE));
        assert_eq!(strukt.next().field(), Ok(0xC0BB0000));
        assert_eq!(strukt.next().next().field(), Ok(0xF0));
        assert_eq!(strukt.next().next().next().field(), Ok("TESTyeet"));
    }
}
