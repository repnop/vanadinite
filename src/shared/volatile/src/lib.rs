// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

use core::cell::UnsafeCell;

#[derive(Debug, Clone, Copy)]
pub struct Read;
#[derive(Debug, Clone, Copy)]
pub struct Write;
#[derive(Debug, Clone, Copy)]
pub struct ReadWrite;

#[derive(Debug)]
#[repr(transparent)]
pub struct Volatile<T, Direction = ReadWrite>(UnsafeCell<T>, core::marker::PhantomData<Direction>);

unsafe impl<T, Dir> Send for Volatile<T, Dir> {}
unsafe impl<T, Dir> Sync for Volatile<T, Dir> {}

impl<T: Copy + 'static> Volatile<T, Read> {
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }
}

impl<T: Copy + 'static> Volatile<T, Write> {
    pub fn write(&self, val: T) {
        unsafe { self.0.get().write_volatile(val) }
    }
}

impl<T: Copy + 'static> Volatile<T, ReadWrite> {
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }

    pub fn write(&self, val: T) {
        unsafe { self.0.get().write_volatile(val) }
    }
}

impl<T: Copy, const N: usize> core::ops::Index<usize> for Volatile<[T; N], Read> {
    type Output = Volatile<T>;

    #[allow(clippy::transmute_ptr_to_ptr)]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
    }
}

impl<T: Copy, const N: usize> core::ops::Index<usize> for Volatile<[T; N], ReadWrite> {
    type Output = Volatile<T>;

    #[allow(clippy::transmute_ptr_to_ptr)]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
    }
}
