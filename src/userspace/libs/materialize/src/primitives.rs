// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod fields;

use crate::{
    hash::FxHasher,
    sealed,
    serialize::{serializers::PrimitiveSerializer, ReservationToken, SerializeError, Serializer},
};
use core::{alloc::Layout, convert::TryFrom};

pub(crate) unsafe trait Integer: Sized + Copy {}
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

pub trait Primitive<'a>: sealed::Sealed + PrimitiveSerializer<'a> + Sized {
    const ID: u64;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError>;
    fn layout() -> Layout;
}

impl<'a> Primitive<'a> for () {
    const ID: u64 = 0xadc4eb49d6e3a43c;

    fn extract(_: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        Ok(())
    }

    fn layout() -> Layout {
        Layout::new::<()>()
    }
}

pub struct Struct<'a, F: Fields<'a>> {
    fields: core::marker::PhantomData<fn() -> F>,
    buffer: AlignedReadBuffer<'a>,
}

impl<'a, F: Fields<'a>> Struct<'a, F> {
    pub const STRUCT_BASE_ID: u64 = 0x8877eea67b715863;

    #[inline]
    pub fn field(&self) -> Result<F::Head, ExtractionError> {
        <F::Head as Primitive>::extract(&mut self.buffer.clone())
    }

    #[inline]
    pub fn next(&self) -> Struct<'a, <F as Fields<'a>>::Next> {
        let mut buffer = self.buffer.clone();
        buffer.position += <<F as Fields>::Head as Primitive>::layout().size();
        Struct { buffer, fields: core::marker::PhantomData }
    }

    #[inline]
    pub fn advance(self) -> Result<(F::Head, Struct<'a, <F as Fields<'a>>::Next>), ExtractionError> {
        Ok((self.field()?, self.next()))
    }
}

impl<'a, F: Fields<'a>> sealed::Sealed for Struct<'a, F> {}
impl<'a, F: Fields<'a>> Primitive<'a> for Struct<'a, F> {
    const ID: u64 = FxHasher::new().hash(Self::STRUCT_BASE_ID).hash(<<F as Fields>::Head as Primitive>::ID).finish();

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        let [id, position] = buffer.read::<[u64; 2]>()?;
        let position = usize::try_from(position).map_err(|_| ExtractionError::MalformedOffset)?;

        if id != Self::ID {
            return Err(ExtractionError::MismatchedId { wanted: Self::ID, found: id });
        } else if buffer.buffer.get(position..position + F::layout().size()).is_none() {
            return Err(ExtractionError::MalformedOffset);
        }

        Ok(Struct { buffer: AlignedReadBuffer { buffer: buffer.buffer, position }, fields: core::marker::PhantomData })
    }

    fn layout() -> Layout {
        Layout::new::<[u64; 2]>()
    }
}

impl sealed::Sealed for &'_ str {}
impl<'a> Primitive<'a> for &'a str {
    const ID: u64 = 0x94a845be7716094d;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;
        let buffer = buffer.buffer.get(position..position + length).ok_or(ExtractionError::MalformedOffset)?;

        if position == 0 {
            return Ok("");
        }

        core::str::from_utf8(buffer).map_err(|_| ExtractionError::InvalidUtf8)
    }

    fn layout() -> Layout {
        Layout::new::<[usize; 2]>()
    }
}

pub struct Array<'a, P: Primitive<'a>, const LENGTH: usize> {
    fields: core::marker::PhantomData<fn() -> P>,
    buffer: AlignedReadBuffer<'a>,
}

impl<'a, P: Primitive<'a>, const LENGTH: usize> Array<'a, P, LENGTH> {
    pub const ARRAY_BASE_ID: u64 = 0xf13a444fbc5162d0;

    #[inline]
    pub fn field(&self) -> Result<P, ExtractionError> {
        <P as Primitive>::extract(&mut self.buffer.clone())
    }

    #[inline]
    pub fn skip<const N: usize>(&self) -> Array<'a, P, { LENGTH - N }> {
        let mut buffer = self.buffer.clone();
        buffer.position += <P as Primitive>::layout().size() * N;
        Array { buffer, fields: core::marker::PhantomData }
    }

    #[inline]
    pub fn pop_front(self) -> Result<(P, Array<'a, P, { LENGTH - 1 }>), ExtractionError> {
        Ok((self.field()?, self.skip::<1>()))
    }

    pub fn nth(&self, n: usize) -> Result<P, ExtractionError> {
        if n >= LENGTH {
            return Err(ExtractionError::MalformedOffset);
        }

        let mut buffer = self.buffer.clone();
        buffer.position += <P as Primitive>::layout().size() * n;
        <P as Primitive>::extract(&mut buffer)
    }

    pub fn map<U>(&self, f: impl Fn(P) -> Result<U, ExtractionError>) -> Result<[U; LENGTH], ExtractionError> {
        let mut i = 0;
        [(); LENGTH]
            .map(|_| {
                let res = self.nth(i).and_then(&f);
                i += 1;
                res
            })
            .try_map(core::convert::identity)
    }
}

impl<'a, const LENGTH: usize, P: Primitive<'a>> sealed::Sealed for Array<'a, P, LENGTH> {}
impl<'a, const LENGTH: usize, P: Primitive<'a>> Primitive<'a> for Array<'a, P, LENGTH> {
    const ID: u64 = FxHasher::new().hash(0xf13a444fbc5162d0).hash(P::ID).hash(LENGTH as u64).finish();

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;

        if buffer.buffer.get(position..position + (P::layout().size() * length)).is_none() {
            return Err(ExtractionError::MalformedOffset);
        }

        Ok(Array { buffer: AlignedReadBuffer { buffer: buffer.buffer, position }, fields: core::marker::PhantomData })
    }

    fn layout() -> Layout {
        Layout::new::<[usize; 2]>()
    }
}

impl sealed::Sealed for u8 {}
impl<'a> Primitive<'a> for u8 {
    const ID: u64 = 0xd4d1d74109db7e0;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i8 {}
impl<'a> Primitive<'a> for i8 {
    const ID: u64 = 0x85316d595ee12d8e;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for u16 {}
impl<'a> Primitive<'a> for u16 {
    const ID: u64 = 0x182ca144e057ded8;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i16 {}
impl<'a> Primitive<'a> for i16 {
    const ID: u64 = 0x8339ca9fef21af4;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for u32 {}
impl<'a> Primitive<'a> for u32 {
    const ID: u64 = 0xb330c6b1bc925fe3;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i32 {}
impl<'a> Primitive<'a> for i32 {
    const ID: u64 = 0xa7618d5014e22dcd;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for u64 {}
impl<'a> Primitive<'a> for u64 {
    const ID: u64 = 0x46f3003d096708b8;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i64 {}
impl<'a> Primitive<'a> for i64 {
    const ID: u64 = 0xf892cc40250d39f7;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for usize {}
impl<'a> Primitive<'a> for usize {
    const ID: u64 = 0x191f7db76a9b101d;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for isize {}
impl<'a> Primitive<'a> for isize {
    const ID: u64 = 0xe14dbb5b71ba5adc;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, ExtractionError> {
        buffer.read::<Self>()
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

pub trait Fields<'a>: Sized + sealed::Sealed {
    const ID: u64;
    type Head: Primitive<'a>;
    type Next: Fields<'a>;

    #[inline(always)]
    fn layout() -> Layout {
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
