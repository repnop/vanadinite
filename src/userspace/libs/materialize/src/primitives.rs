// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod fields;

use crate::{
    deserialize::DeserializeError, hash::FxHasher, sealed, serialize::serializers::PrimitiveSerializer, Deserialize,
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
    fn read<I: Integer>(&mut self) -> Result<I, DeserializeError> {
        let buffer = self.buffer.get(self.position..).ok_or(DeserializeError::BufferTooSmall)?;
        let (pad, slice, _) = unsafe { buffer.align_to::<I>() };
        match slice {
            [] => Err(DeserializeError::BufferTooSmall),
            [value, ..] => {
                self.position += pad.len() + core::mem::size_of::<I>();
                Ok(*value)
            }
        }
    }
}

pub trait Primitive<'a>: sealed::Sealed + PrimitiveSerializer<'a> + Sized {
    const ID: u64;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError>;
    fn layout() -> Layout;
}

impl<'a> Primitive<'a> for () {
    const ID: u64 = 0xadc4eb49d6e3a43c;

    fn extract(_: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
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
    pub fn field(&self) -> Result<F::Head, DeserializeError> {
        <F::Head as Primitive>::extract(&mut self.buffer.clone())
    }

    #[inline]
    pub fn next(&self) -> Struct<'a, <F as Fields<'a>>::Next> {
        let mut buffer = self.buffer.clone();
        buffer.position += <<F as Fields>::Head as Primitive>::layout().size();
        Struct { buffer, fields: core::marker::PhantomData }
    }

    #[inline]
    pub fn advance(self) -> Result<(F::Head, Struct<'a, <F as Fields<'a>>::Next>), DeserializeError> {
        Ok((self.field()?, self.next()))
    }
}

impl<'a, F: Fields<'a>> sealed::Sealed for Struct<'a, F> {}
impl<'a, F: Fields<'a>> Primitive<'a> for Struct<'a, F> {
    const ID: u64 = FxHasher::new().hash(Self::STRUCT_BASE_ID).hash(<<F as Fields>::Head as Primitive>::ID).finish();

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        let [id, position] = buffer.read::<[u64; 2]>()?;
        let position = usize::try_from(position).map_err(|_| DeserializeError::MalformedOffset)?;

        if id != Self::ID {
            return Err(DeserializeError::MismatchedId { wanted: Self::ID, found: id });
        } else if buffer.buffer.get(position..position + F::layout().size()).is_none() {
            return Err(DeserializeError::MalformedOffset);
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

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;
        let buffer = buffer.buffer.get(position..position + length).ok_or(DeserializeError::MalformedOffset)?;

        if position == 0 {
            return Ok("");
        }

        core::str::from_utf8(buffer).map_err(|_| DeserializeError::InvalidUtf8)
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
    pub fn field(&self) -> Result<P, DeserializeError> {
        <P as Primitive>::extract(&mut self.buffer.clone())
    }

    #[inline]
    pub fn skip<const N: usize>(&self) -> Array<'a, P, { LENGTH - N }> {
        let mut buffer = self.buffer.clone();
        buffer.position += <P as Primitive>::layout().size() * N;
        Array { buffer, fields: core::marker::PhantomData }
    }

    #[inline]
    pub fn pop_front(self) -> Result<(P, Array<'a, P, { LENGTH - 1 }>), DeserializeError> {
        Ok((self.field()?, self.skip::<1>()))
    }

    pub fn nth(&self, n: usize) -> Result<P, DeserializeError> {
        if n >= LENGTH {
            return Err(DeserializeError::MalformedOffset);
        }

        let mut buffer = self.buffer.clone();
        buffer.position += <P as Primitive>::layout().size() * n;
        <P as Primitive>::extract(&mut buffer)
    }

    pub fn map<U>(&self, f: impl Fn(P) -> Result<U, DeserializeError>) -> Result<[U; LENGTH], DeserializeError> {
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
    const ID: u64 = FxHasher::new().hash(Self::ARRAY_BASE_ID).hash(P::ID).hash(LENGTH as u64).finish();

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;

        if buffer
            .buffer
            .get(
                position
                    ..position
                        + (P::layout()
                            .repeat(length)
                            .map_err(|_| DeserializeError::BufferTooSmall)?
                            .0
                            .pad_to_align()
                            .size()),
            )
            .is_none()
        {
            return Err(DeserializeError::MalformedOffset);
        }

        Ok(Array { buffer: AlignedReadBuffer { buffer: buffer.buffer, position }, fields: core::marker::PhantomData })
    }

    fn layout() -> Layout {
        Layout::new::<[usize; 2]>()
    }
}

pub struct List<'a, P: Primitive<'a>> {
    fields: core::marker::PhantomData<fn() -> P>,
    length: usize,
    buffer: AlignedReadBuffer<'a>,
}

impl<'a, P: Primitive<'a> + 'a> List<'a, P> {
    pub fn into_iter(mut self) -> impl Iterator<Item = Result<P, DeserializeError>> + 'a {
        core::iter::from_fn(move || match self.pop_front().transpose()? {
            Ok(next) => Some(Ok(next)),
            Err(e) => Some(Err(e)),
        })
    }

    pub fn pop_front(&mut self) -> Result<Option<P>, DeserializeError> {
        if self.length == 0 {
            return Ok(None);
        }

        let p = P::extract(&mut self.buffer.clone())?;
        self.buffer.position += <P as Primitive>::layout().size();
        self.length -= 1;
        Ok(Some(p))
    }
}

impl<'a, P: Primitive<'a>> sealed::Sealed for List<'a, P> {}
impl<'a, P: Primitive<'a>> Primitive<'a> for List<'a, P> {
    const ID: u64 = FxHasher::new().hash(0xf3685126faa78352).hash(P::ID).finish();

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;

        if buffer
            .buffer
            .get(
                position
                    ..position
                        + (P::layout()
                            .repeat(length)
                            .map_err(|_| DeserializeError::BufferTooSmall)?
                            .0
                            .pad_to_align()
                            .size()),
            )
            .is_none()
        {
            return Err(DeserializeError::MalformedOffset);
        }

        Ok(List {
            buffer: AlignedReadBuffer { buffer: buffer.buffer, position },
            length,
            fields: core::marker::PhantomData,
        })
    }

    fn layout() -> Layout {
        Layout::new::<[usize; 2]>()
    }
}

pub struct Enum<'a, DISCRIMINANT: Primitive<'a>> {
    associated_data_id: u64,
    associated_data_position: usize,
    buffer: AlignedReadBuffer<'a>,
    discriminant: core::marker::PhantomData<fn() -> DISCRIMINANT>,
}

impl<'a, DISCRIMINANT: Primitive<'a>> Enum<'a, DISCRIMINANT> {
    pub const ENUM_BASE_ID: u64 = 0xd60d20b6f2424b0f;

    pub fn discriminant(&self) -> Result<DISCRIMINANT, DeserializeError> {
        DISCRIMINANT::extract(&mut self.buffer.clone())
    }

    pub fn associated_data<P: Primitive<'a>>(self) -> Result<P, DeserializeError> {
        if P::ID != self.associated_data_id {
            return Err(DeserializeError::MismatchedId { wanted: self.associated_data_id, found: P::ID });
        }

        P::extract(&mut AlignedReadBuffer { buffer: self.buffer.buffer, position: self.associated_data_position })
    }
}

impl<'a, DISCRIMINANT: Primitive<'a>> sealed::Sealed for Enum<'a, DISCRIMINANT> {}
impl<'a, DISCRIMINANT: Primitive<'a>> Primitive<'a> for Enum<'a, DISCRIMINANT> {
    const ID: u64 = FxHasher::new().hash(Self::ENUM_BASE_ID).hash(DISCRIMINANT::ID).finish();

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        let [id, associated_data_id, associated_data_position] = buffer.read::<[u64; 3]>()?;
        let associated_data_position =
            usize::try_from(associated_data_position).map_err(|_| DeserializeError::MalformedOffset)?;

        if id != Self::ID {
            return Err(DeserializeError::MismatchedId { wanted: Self::ID, found: id });
        }

        let our_buffer = buffer.clone();
        // FIXME: need to add padding size here? don't think so but
        buffer.position += DISCRIMINANT::layout().size();

        Ok(Self {
            associated_data_position,
            associated_data_id,
            buffer: our_buffer,
            discriminant: core::marker::PhantomData,
        })
    }

    fn layout() -> Layout {
        Layout::new::<[u64; 3]>().extend(DISCRIMINANT::layout()).unwrap().0.pad_to_align()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Capability {
    pub index: usize,
}

impl sealed::Sealed for Capability {}
impl<'a> Primitive<'a> for Capability {
    const ID: u64 = 0xe6803b0bbf7d6641;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        Ok(Self { index: buffer.read()? })
    }

    fn layout() -> Layout {
        Layout::new::<usize>()
    }
}

impl sealed::Sealed for u8 {}
impl<'a> Primitive<'a> for u8 {
    const ID: u64 = 0xd4d1d74109db7e0;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i8 {}
impl<'a> Primitive<'a> for i8 {
    const ID: u64 = 0x85316d595ee12d8e;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for u16 {}
impl<'a> Primitive<'a> for u16 {
    const ID: u64 = 0x182ca144e057ded8;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i16 {}
impl<'a> Primitive<'a> for i16 {
    const ID: u64 = 0x8339ca9fef21af4;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for u32 {}
impl<'a> Primitive<'a> for u32 {
    const ID: u64 = 0xb330c6b1bc925fe3;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i32 {}
impl<'a> Primitive<'a> for i32 {
    const ID: u64 = 0xa7618d5014e22dcd;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for u64 {}
impl<'a> Primitive<'a> for u64 {
    const ID: u64 = 0x46f3003d096708b8;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for i64 {}
impl<'a> Primitive<'a> for i64 {
    const ID: u64 = 0xf892cc40250d39f7;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for usize {}
impl<'a> Primitive<'a> for usize {
    const ID: u64 = 0x191f7db76a9b101d;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
        buffer.read::<Self>()
    }

    fn layout() -> Layout {
        Layout::new::<Self>()
    }
}

impl sealed::Sealed for isize {}
impl<'a> Primitive<'a> for isize {
    const ID: u64 = 0xe14dbb5b71ba5adc;

    fn extract(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self, DeserializeError> {
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
