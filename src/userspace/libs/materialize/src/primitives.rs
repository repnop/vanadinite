// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod sealed {
    pub trait Sealed {}
}

pub struct PrimitiveBuffer<'a> {
    pub(crate) buffer: &'a [u8],
}

pub enum ExtractionError {}

pub trait Primitive: sealed::Sealed {
    type Output<'a>;

    fn extract(buffer: PrimitiveBuffer<'_>) -> Result<Self::Output<'_>, ExtractionError>;
    fn size() -> usize;
}

impl Primitive for () {
    type Output<'a> = ();

    fn extract(_: PrimitiveBuffer<'_>) -> Result<Self::Output<'_>, ExtractionError> {
        Ok(())
    }

    fn size() -> usize {
        0
    }
}

#[repr(transparent)]
pub struct Struct<F: Fields> {
    fields: core::marker::PhantomData<fn() -> F>,
    buffer: [u8],
}

impl<F: Fields> Struct<F> {
    pub fn field(&self) -> Result<<F::Head as Primitive>::Output<'_>, ExtractionError> {
        <F::Head as Primitive>::extract(PrimitiveBuffer { buffer: &self.buffer })
    }

    pub fn next(&self) -> &'_ Struct<<F as Fields>::Next> {
        // SAFETY:
        // `Struct<F>` is `#[repr(transparent)]` around a `[u8]`
        unsafe {
            &*(self.buffer.get(..<<F as Fields>::Head as Primitive>::size()).unwrap_or_default() as *const [u8]
                as *const Struct<<F as Fields>::Next>)
        }
    }
}

impl<F: Fields> sealed::Sealed for Struct<F> {}
impl<F: Fields + 'static> Primitive for Struct<F> {
    type Output<'a> = &'a Struct<F>;

    fn extract(buffer: PrimitiveBuffer<'_>) -> Result<Self::Output<'_>, ExtractionError> {
        // This doesn't compile obviously...
        // Ok(Self { buffer, fields: core::marker::PhantomData })
        todo!()
    }

    fn size() -> usize {
        <F as Fields>::size()
    }
}

pub trait Fields: Sized + sealed::Sealed {
    type Head: Primitive;
    type Next: Fields;

    fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}

impl<T: Primitive> sealed::Sealed for (T,) {}
impl<T: Primitive> Fields for (T,) {
    type Head = T;
    type Next = ();
}

impl<T: Primitive, U: Primitive> sealed::Sealed for (T, U) {}
impl<T: Primitive, U: Primitive> Fields for (T, U) {
    type Head = T;
    type Next = (U,);
}

impl sealed::Sealed for () {}
impl Fields for () {
    type Head = ();
    type Next = ();
}
