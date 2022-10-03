// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::hash::FxHasher;
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
    type Output<'a>;

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError>;
    fn layout() -> core::alloc::Layout;
}

impl Primitive for () {
    const ID: u64 = 0xadc4eb49d6e3a43c;
    type Output<'a> = ();

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
    type Output<'a> = Struct<'a, F>;

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
    type Output<'a> = &'a str;

    fn extract<'a>(buffer: &mut AlignedReadBuffer<'a>) -> Result<Self::Output<'a>, ExtractionError> {
        let [position, length] = buffer.read::<[usize; 2]>()?;
        let buffer = buffer.buffer.get(position..position + length).ok_or(ExtractionError::MalformedOffset)?;

        core::str::from_utf8(buffer).map_err(|_| ExtractionError::InvalidUtf8)
    }

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<[u64; 2]>()
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
    const ID: u64 = FxHasher::new().hash(0xf13a444fbc5162d0).hash(P::ID).finish();
    type Output<'a> = Array<'a, P, LENGTH>;

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
    type Output<'a> = u8;

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
    type Output<'a> = i8;

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
    type Output<'a> = u16;

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
    type Output<'a> = i16;

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
    type Output<'a> = u32;

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
    type Output<'a> = i32;

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
    type Output<'a> = u64;

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
    type Output<'a> = i64;

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
    type Output<'a> = usize;

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
    type Output<'a> = isize;

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

    fn layout() -> core::alloc::Layout {
        <Self::Head as Primitive>::layout().extend(<Self::Next as Fields>::layout()).unwrap().0.pad_to_align()
    }
}

impl sealed::Sealed for () {}
impl Fields for () {
    const ID: u64 = <() as Primitive>::ID;
    type Head = ();
    type Next = ();

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<()>()
    }
}

impl<T: Primitive> sealed::Sealed for (T,) {}
impl<T: Primitive> Fields for (T,) {
    const ID: u64 = T::ID;
    type Head = T;
    type Next = ();
}

impl<T: Primitive, U: Primitive> sealed::Sealed for (T, U) {}
impl<T: Primitive, U: Primitive> Fields for (T, U) {
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U,);
}

impl<T: Primitive, U: Primitive, V: Primitive> sealed::Sealed for (T, U, V) {}
impl<T: Primitive, U: Primitive, V: Primitive> Fields for (T, U, V) {
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V);
}

impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive> sealed::Sealed for (T, U, V, W) {}
impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive> Fields for (T, U, V, W) {
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W);
}

impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive, X: Primitive> sealed::Sealed for (T, U, V, W, X) {}
impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive, X: Primitive> Fields for (T, U, V, W, X) {
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X);
}

impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive, X: Primitive, Y: Primitive> sealed::Sealed
    for (T, U, V, W, X, Y)
{
}
impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive, X: Primitive, Y: Primitive> Fields for (T, U, V, W, X, Y) {
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y);
}

impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive, X: Primitive, Y: Primitive, Z: Primitive> sealed::Sealed
    for (T, U, V, W, X, Y, Z)
{
}
impl<T: Primitive, U: Primitive, V: Primitive, W: Primitive, X: Primitive, Y: Primitive, Z: Primitive> Fields
    for (T, U, V, W, X, Y, Z)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D, E)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D, E)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D, E);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D, E, F)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D, E, F)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D, E, F);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D, E, F, G);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G, H)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G, H)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D, E, F, G, H);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G, H, I)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G, H, I)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D, E, F, G, H, I);
}

impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
    > sealed::Sealed for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G, H, I, J)
{
}
impl<
        T: Primitive,
        U: Primitive,
        V: Primitive,
        W: Primitive,
        X: Primitive,
        Y: Primitive,
        Z: Primitive,
        A: Primitive,
        B: Primitive,
        C: Primitive,
        D: Primitive,
        E: Primitive,
        F: Primitive,
        G: Primitive,
        H: Primitive,
        I: Primitive,
        J: Primitive,
    > Fields for (T, U, V, W, X, Y, Z, A, B, C, D, E, F, G, H, I, J)
{
    const ID: u64 = FxHasher::new().hash(T::ID).hash(<Self::Next as Fields>::ID).finish();
    type Head = T;
    type Next = (U, V, W, X, Y, Z, A, B, C, D, E, F, G, H, I, J);
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
