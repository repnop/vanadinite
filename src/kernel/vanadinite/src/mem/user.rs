// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::csr::sstatus::TemporaryUserMemoryAccess;

use super::{
    manager::{InvalidRegion, MemoryManager},
    paging::{
        flags::{self, Flags},
        VirtualAddress,
    },
};
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy)]
pub enum InvalidUserPtr {
    InvalidAccess,
    NotMapped,
    Unaligned,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RawUserPtr<Mode: UserPtrMode, T> {
    addr: VirtualAddress,
    typë: PhantomData<*mut T>,
    mode: PhantomData<Mode>,
}

impl<Mode: UserPtrMode, T> RawUserPtr<Mode, T> {
    pub fn new(addr: VirtualAddress) -> Self {
        Self { addr, typë: PhantomData, mode: PhantomData }
    }

    /// # Safety
    /// The provided [`MemoryManager`] must be the memory manager of the current
    /// task, otherwise the [`ValidatedUserPtr`] could create invalid references
    /// into the current address space
    ///
    /// Validates the [`RawUserPtr`] against the specified type and access mode
    pub unsafe fn validate(self, manager: &MemoryManager) -> Result<ValidatedUserPtr<Mode, T>, InvalidUserPtr> {
        if self.addr.as_usize() % core::mem::align_of::<T>() != 0 {
            return Err(InvalidUserPtr::Unaligned);
        }

        let addr_range = self.addr..self.addr.add(core::mem::size_of::<T>());

        match manager.is_user_region_valid(addr_range, |f| f & Mode::FLAGS) {
            Ok(_) => Ok(ValidatedUserPtr { addr: self.addr, typë: self.typë, mode: self.mode }),
            Err((_, InvalidRegion::NotMapped)) => Err(InvalidUserPtr::NotMapped),
            Err((_, InvalidRegion::InvalidPermissions)) => Err(InvalidUserPtr::InvalidAccess),
        }
    }
}

impl<T> RawUserPtr<Read, T> {
    pub fn readable(addr: VirtualAddress) -> Self {
        Self { addr, typë: PhantomData, mode: PhantomData }
    }
}

impl<T> RawUserPtr<ReadWrite, T> {
    pub fn writable(addr: VirtualAddress) -> Self {
        Self { addr, typë: PhantomData, mode: PhantomData }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ValidatedUserPtr<Mode: UserPtrMode, T> {
    addr: VirtualAddress,
    typë: PhantomData<*mut T>,
    mode: PhantomData<Mode>,
}

impl<T: Copy> ValidatedUserPtr<Read, T> {
    pub fn read(&self) -> T {
        let _guard = TemporaryUserMemoryAccess::new();
        unsafe { *self.addr.as_ptr().cast() }
    }
}

impl<T> ValidatedUserPtr<Read, T> {
    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        let _guard = TemporaryUserMemoryAccess::new();
        f(unsafe { &*self.addr.as_ptr().cast() })
    }
}

impl<T: Copy> ValidatedUserPtr<Read, T> {
    pub fn write(&mut self, value: T) {
        let _guard = TemporaryUserMemoryAccess::new();
        unsafe { *self.addr.as_mut_ptr().cast() = value };
    }
}

impl<T> ValidatedUserPtr<ReadWrite, T> {
    pub fn with<U>(&mut self, f: impl FnOnce(&mut T) -> U) -> U {
        let _guard = TemporaryUserMemoryAccess::new();
        f(unsafe { &mut *self.addr.as_mut_ptr().cast() })
    }
}

impl<Mode: UserPtrMode, T> ValidatedUserPtr<Mode, T> {
    pub fn guarded(&self) -> ValidUserPtrGuard<'_, Mode, T> {
        ValidUserPtrGuard::new(self)
    }
}

pub struct ValidUserPtrGuard<'a, Mode: UserPtrMode, T> {
    valid_ptr: &'a ValidatedUserPtr<Mode, T>,
    _guard: TemporaryUserMemoryAccess,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a, Mode: UserPtrMode, T> ValidUserPtrGuard<'a, Mode, T> {
    fn new(valid_ptr: &'a ValidatedUserPtr<Mode, T>) -> Self {
        Self { valid_ptr, _guard: TemporaryUserMemoryAccess::new(), _marker: PhantomData }
    }
}

impl<T> core::ops::Deref for ValidUserPtrGuard<'_, Read, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.valid_ptr.addr.as_ptr().cast() }
    }
}

impl<T> core::ops::Deref for ValidUserPtrGuard<'_, ReadWrite, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.valid_ptr.addr.as_ptr().cast() }
    }
}

impl<T> core::ops::DerefMut for ValidUserPtrGuard<'_, ReadWrite, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.valid_ptr.addr.as_mut_ptr().cast() }
    }
}

pub trait UserPtrMode {
    const FLAGS: Flags;
}

pub struct Read;
impl UserPtrMode for Read {
    const FLAGS: Flags = flags::READ;
}

pub struct ReadWrite;
impl UserPtrMode for ReadWrite {
    const FLAGS: Flags = Flags::new(flags::READ.value() | flags::WRITE.value());
}

#[derive(Debug)]
pub struct RawUserSlice<Mode: UserPtrMode, T> {
    addr: VirtualAddress,
    len: usize,
    typë: PhantomData<*mut T>,
    mode: PhantomData<Mode>,
}

impl<Mode: UserPtrMode, T> RawUserSlice<Mode, T> {
    pub fn new(addr: VirtualAddress, len: usize) -> Self {
        Self { addr, len, typë: PhantomData, mode: PhantomData }
    }

    /// # Safety
    /// The provided [`MemoryManager`] must be the memory manager of the current
    /// task, otherwise the [`ValidatedUserSlice`] could create invalid references
    /// into the current address space
    ///
    /// Validates the [`RawUserSlice`] against the specified type and access mode
    pub unsafe fn validate(
        self,
        manager: &MemoryManager,
    ) -> Result<ValidatedUserSlice<Mode, T>, (VirtualAddress, InvalidUserPtr)> {
        if self.addr.as_usize() % core::mem::align_of::<T>() != 0 {
            return Err((self.addr, InvalidUserPtr::Unaligned));
        }

        if self.len == 0 {
            // FIXME: I think this is fine?
            return Ok(ValidatedUserSlice { addr: self.addr, len: self.len, typë: self.typë, mode: self.mode });
        }

        let addr_range = self.addr..self.addr.add(core::mem::size_of::<T>() * self.len);

        match manager.is_user_region_valid(addr_range, |f| f & Mode::FLAGS) {
            Ok(_) => Ok(ValidatedUserSlice { addr: self.addr, len: self.len, typë: self.typë, mode: self.mode }),
            Err((addr, InvalidRegion::NotMapped)) => Err((addr, InvalidUserPtr::NotMapped)),
            Err((addr, InvalidRegion::InvalidPermissions)) => Err((addr, InvalidUserPtr::InvalidAccess)),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn addr(&self) -> VirtualAddress {
        self.addr
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> RawUserSlice<Read, T> {
    pub fn readable(addr: VirtualAddress, len: usize) -> Self {
        Self { addr, len, typë: PhantomData, mode: PhantomData }
    }
}

impl<T> RawUserSlice<ReadWrite, T> {
    pub fn writable(addr: VirtualAddress, len: usize) -> Self {
        Self { addr, len, typë: PhantomData, mode: PhantomData }
    }
}

unsafe impl<Mode: UserPtrMode, T> Send for RawUserSlice<Mode, T> {}

#[derive(Debug)]
pub struct ValidatedUserSlice<Mode: UserPtrMode, T> {
    addr: VirtualAddress,
    len: usize,
    typë: PhantomData<*mut T>,
    mode: PhantomData<Mode>,
}

impl<Mode: UserPtrMode, T> ValidatedUserSlice<Mode, T> {
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T> ValidatedUserSlice<Read, T> {
    pub fn with<U>(&self, f: impl FnOnce(&[T]) -> U) -> U {
        let _guard = TemporaryUserMemoryAccess::new();
        f(unsafe { core::slice::from_raw_parts(self.addr.as_ptr().cast(), self.len) })
    }
}

impl<T> ValidatedUserSlice<ReadWrite, T> {
    pub fn with<U>(&mut self, f: impl FnOnce(&mut [T]) -> U) -> U {
        let _guard = TemporaryUserMemoryAccess::new();
        f(unsafe { core::slice::from_raw_parts_mut(self.addr.as_mut_ptr().cast(), self.len) })
    }
}

impl<Mode: UserPtrMode, T> ValidatedUserSlice<Mode, T> {
    pub fn guarded(&self) -> ValidUserSliceGuard<'_, Mode, T> {
        ValidUserSliceGuard::new(self)
    }
}

pub struct ValidUserSliceGuard<'a, Mode: UserPtrMode, T> {
    valid_slice: &'a ValidatedUserSlice<Mode, T>,
    _guard: TemporaryUserMemoryAccess,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a, Mode: UserPtrMode, T> ValidUserSliceGuard<'a, Mode, T> {
    fn new(valid_slice: &'a ValidatedUserSlice<Mode, T>) -> Self {
        Self { valid_slice, _guard: TemporaryUserMemoryAccess::new(), _marker: PhantomData }
    }
}

impl<T> core::ops::Deref for ValidUserSliceGuard<'_, Read, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.valid_slice.addr.as_ptr().cast(), self.valid_slice.len) }
    }
}

impl<T> core::ops::Deref for ValidUserSliceGuard<'_, ReadWrite, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.valid_slice.addr.as_ptr().cast(), self.valid_slice.len) }
    }
}

impl<T> core::ops::DerefMut for ValidUserSliceGuard<'_, ReadWrite, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.valid_slice.addr.as_mut_ptr().cast(), self.valid_slice.len) }
    }
}
