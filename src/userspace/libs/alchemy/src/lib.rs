// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![allow(incomplete_features)]
#![feature(arbitrary_self_types, generic_const_exprs)]

pub use alchemy_derive::PackedStruct;

#[derive(Debug, Clone, Copy)]
pub enum TryCastError {
    NotLongEnough,
    Underaligned,
}

/// # Safety
/// This trait should only be implemented if all bit patterns are valid for the
/// type
pub unsafe trait OnlyValidBitPatterns: Sized + Copy {
    fn zeroed() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

unsafe impl OnlyValidBitPatterns for u8 {}
unsafe impl OnlyValidBitPatterns for u16 {}
unsafe impl OnlyValidBitPatterns for u32 {}
unsafe impl OnlyValidBitPatterns for u64 {}
unsafe impl OnlyValidBitPatterns for usize {}
unsafe impl OnlyValidBitPatterns for i8 {}
unsafe impl OnlyValidBitPatterns for i16 {}
unsafe impl OnlyValidBitPatterns for i32 {}
unsafe impl OnlyValidBitPatterns for i64 {}
unsafe impl OnlyValidBitPatterns for isize {}
unsafe impl<T: OnlyValidBitPatterns, const N: usize> OnlyValidBitPatterns for [T; N] {}

#[doc(hidden)]
pub struct If<const B: bool>;
#[doc(hidden)]
pub trait True {}
impl True for If<true> {}

pub const fn valid_cast_align_size(align1: usize, align2: usize, size1: usize, size2: usize) -> bool {
    align1 >= align2 && size1 >= size2
}

/// # Safety
/// This trait must only be implemented _iff_:
/// 1. The struct has no internal or end padding
/// 2. The struct is `#[repr(C)]` or `#[repr(transparent)]`
/// 3. All types within the struct also `impl PackedStruct`
pub unsafe trait PackedStruct: OnlyValidBitPatterns {
    fn cast<U: PackedStruct>(self) -> U
    where
        If<{ core::mem::size_of::<Self>() >= core::mem::size_of::<U>() }>: True,
    {
        unsafe { core::mem::transmute_copy(&self) }
    }

    fn cast_ref<U: PackedStruct>(&self) -> &U
    where
        If<
            {
                valid_cast_align_size(
                    core::mem::align_of::<Self>(),
                    core::mem::align_of::<U>(),
                    core::mem::size_of::<Self>(),
                    core::mem::size_of::<U>(),
                )
            },
        >: True,
    {
        unsafe { &*(self as *const _ as *const U) }
    }

    fn cast_mut_ref<U: PackedStruct>(&mut self) -> &mut U
    where
        If<
            {
                valid_cast_align_size(
                    core::mem::align_of::<Self>(),
                    core::mem::align_of::<U>(),
                    core::mem::size_of::<Self>(),
                    core::mem::size_of::<U>(),
                )
            },
        >: True,
    {
        unsafe { &mut *(self as *mut _ as *mut U) }
    }

    fn into_bytes(self) -> [u8; core::mem::size_of::<Self>()] {
        unsafe { core::mem::transmute_copy(&self) }
    }

    fn as_bytes(&self) -> &[u8; core::mem::size_of::<Self>()] {
        unsafe { &*(self as *const _ as *const [u8; core::mem::size_of::<Self>()]) }
    }

    fn from_bytes(bytes: [u8; core::mem::size_of::<Self>()]) -> Self {
        unsafe { core::mem::transmute_copy(&bytes) }
    }

    fn from_bytes_ref<const N: usize>(bytes: &[u8; N]) -> &Self
    where
        If<
            {
                valid_cast_align_size(
                    core::mem::align_of::<u8>(),
                    core::mem::align_of::<Self>(),
                    core::mem::size_of::<[u8; N]>(),
                    core::mem::size_of::<Self>(),
                )
            },
        >: True,
    {
        unsafe { &*(bytes.as_ptr().cast::<Self>()) }
    }

    fn from_bytes_mut<const N: usize>(bytes: &mut [u8; N]) -> &mut Self
    where
        If<
            {
                valid_cast_align_size(
                    core::mem::align_of::<u8>(),
                    core::mem::align_of::<Self>(),
                    core::mem::size_of::<[u8; N]>(),
                    core::mem::size_of::<Self>(),
                )
            },
        >: True,
    {
        unsafe { &mut *(bytes.as_mut_ptr().cast::<Self>()) }
    }

    fn try_from_byte_slice(slice: &[u8]) -> Result<&Self, TryCastError> {
        if slice.as_ptr() as usize % core::mem::align_of::<Self>() != 0 {
            return Err(TryCastError::Underaligned);
        }

        if slice.len() / core::mem::size_of::<Self>() == 0 {
            return Err(TryCastError::NotLongEnough);
        }

        Ok(unsafe { &*(slice.as_ptr().cast::<Self>()) })
    }

    fn try_from_mut_byte_slice(slice: &mut [u8]) -> Result<&mut Self, TryCastError> {
        if slice.as_mut_ptr() as usize % core::mem::align_of::<Self>() != 0 {
            return Err(TryCastError::Underaligned);
        }

        if slice.len() / core::mem::size_of::<Self>() == 0 {
            return Err(TryCastError::NotLongEnough);
        }

        Ok(unsafe { &mut *(slice.as_mut_ptr().cast::<Self>()) })
    }

    fn cast_slice<U: PackedStruct>(this: &[Self]) -> &[U]
    where
        If<{ core::mem::align_of::<Self>() >= core::mem::align_of::<U>() }>: True,
    {
        let num_us = this.len() * core::mem::size_of::<Self>() / core::mem::size_of::<U>();
        unsafe { core::slice::from_raw_parts(this.as_ptr().cast(), num_us) }
    }

    fn try_cast_slice<U: PackedStruct>(this: &[Self]) -> Result<&[U], TryCastError> {
        let address = this.as_ptr() as usize;
        match address % core::mem::align_of::<U>() == 0 {
            true => {
                let num_us = this.len() * core::mem::size_of::<Self>() / core::mem::size_of::<U>();
                Ok(unsafe { core::slice::from_raw_parts(this.as_ptr().cast(), num_us) })
            }
            false => Err(TryCastError::Underaligned),
        }
    }

    fn bytes_of_slice(this: &[Self]) -> &[u8] {
        unsafe { core::slice::from_raw_parts(this.as_ptr().cast(), this.len() * core::mem::size_of::<Self>()) }
    }

    fn cast_slice_mut<U: PackedStruct>(this: &mut [Self]) -> &mut [U]
    where
        If<{ core::mem::align_of::<Self>() >= core::mem::align_of::<U>() }>: True,
    {
        let num_us = this.len() * core::mem::size_of::<Self>() / core::mem::size_of::<U>();
        unsafe { core::slice::from_raw_parts_mut(this.as_mut_ptr().cast(), num_us) }
    }

    fn bytes_of_slice_mut(this: &mut [Self]) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(this.as_mut_ptr().cast(), this.len() * core::mem::size_of::<Self>()) }
    }
}

unsafe impl PackedStruct for u8 {}
unsafe impl PackedStruct for u16 {}
unsafe impl PackedStruct for u32 {}
unsafe impl PackedStruct for u64 {}
unsafe impl PackedStruct for usize {}
unsafe impl PackedStruct for i8 {}
unsafe impl PackedStruct for i16 {}
unsafe impl PackedStruct for i32 {}
unsafe impl PackedStruct for i64 {}
unsafe impl PackedStruct for isize {}
unsafe impl<T: PackedStruct, const N: usize> PackedStruct for [T; N] {}
