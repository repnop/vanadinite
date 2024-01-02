// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::CapabilityPtr,
    error::SyscallError,
    syscalls::mem::{allocate_device_addressable_memory, DmaAllocationOptions, MemoryPermissions},
    units::Bytes,
};
use core::{
    mem::MaybeUninit,
    ptr::{NonNull, Pointee},
};

pub struct SharedMemoryAllocation {
    pub cptr: CapabilityPtr,
    ptr: NonNull<[u8]>,
}

impl SharedMemoryAllocation {
    pub fn new(size: Bytes, permissions: MemoryPermissions) -> Result<Self, SyscallError> {
        let (cptr, ptr) = crate::syscalls::mem::allocate_shared_memory(size, permissions)?;

        Ok(Self {
            cptr,
            // SAFETY: The kernel will never return us a null pointer if the
            // syscall succeeds
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        })
    }

    pub fn public_rw(size: Bytes) -> Result<Self, SyscallError> {
        Self::new(size, MemoryPermissions::READ | MemoryPermissions::WRITE)
    }

    pub fn private_rw(size: Bytes) -> Result<Self, SyscallError> {
        Self::new(size, MemoryPermissions::READ | MemoryPermissions::WRITE).map(|mut s| {
            s.cptr = CapabilityPtr::from_raw(usize::MAX);
            s
        })
    }

    pub unsafe fn as_mut(&mut self) -> &mut [u8] {
        self.ptr.as_mut()
    }

    pub unsafe fn as_ref(&self) -> &[u8] {
        self.ptr.as_ref()
    }

    pub fn into_parts(self) -> (CapabilityPtr, NonNull<[u8]>) {
        (self.cptr, self.ptr)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FenceMode {
    Full,
    Read,
    Write,
}

#[inline(always)]
pub fn fence(mode: FenceMode) {
    match mode {
        FenceMode::Full => unsafe { core::arch::asm!("fence iorw, iorw") },
        FenceMode::Read => unsafe { core::arch::asm!("fence ir, ir") },
        FenceMode::Write => unsafe { core::arch::asm!("fence ow, ow") },
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(addr: usize) -> Self {
        PhysicalAddress(addr)
    }

    pub fn as_ptr(self) -> *const u8 {
        self.0 as *const u8
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn as_mut_ptr(self) -> *mut u8 {
        self.0 as *mut u8
    }

    #[must_use]
    pub fn offset(self, offset: usize) -> Self {
        Self(self.0 + offset)
    }
}

impl core::fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysicalAddress({:#p})", self.0 as *const u8)
    }
}

impl core::fmt::Pointer for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&(self.0 as *const u8), f)
    }
}

pub struct DmaRegion<T: ?Sized> {
    phys: PhysicalAddress,
    virt: NonNull<T>,
}

impl<T: Sized> DmaRegion<[MaybeUninit<T>]> {
    pub fn new_many(n_elements: usize) -> Result<Self, SyscallError> {
        allocate_device_addressable_memory(Bytes(n_elements * core::mem::size_of::<T>()), DmaAllocationOptions::NONE)
            .map(|(phys, virt)| Self { phys, virt: NonNull::slice_from_raw_parts(virt.cast(), n_elements) })
    }

    pub unsafe fn zeroed_many(n_elements: usize) -> Result<Self, SyscallError> {
        allocate_device_addressable_memory(Bytes(n_elements * core::mem::size_of::<T>()), DmaAllocationOptions::ZERO)
            .map(|(phys, virt)| Self { phys, virt: NonNull::slice_from_raw_parts(virt.cast(), n_elements) })
    }

    pub unsafe fn assume_init(self) -> DmaRegion<[T]> {
        let phys = self.phys;
        let virt = self.virt;
        core::mem::forget(self);

        DmaRegion { phys, virt: NonNull::slice_from_raw_parts(virt.cast(), virt.len()) }
    }
}

impl<T: Sized> DmaRegion<[T]> {
    pub fn get(&mut self, index: usize) -> Option<DmaElement<'_, T>> {
        if index < self.virt.len() {
            Some(DmaElement {
                phys: PhysicalAddress::new(self.phys.0 + core::mem::size_of::<T>() * index),
                virt: unsafe { NonNull::new_unchecked(self.virt.as_ptr().get_unchecked_mut(index)) },
                _region: self,
            })
        } else {
            None
        }
    }
}

impl<T: ?Sized> DmaRegion<T> {
    pub unsafe fn new_raw(metadata: <T as Pointee>::Metadata, zero: bool) -> Result<Self, SyscallError> {
        let size = core::mem::size_of_val_raw::<T>(core::ptr::from_raw_parts(core::ptr::null(), metadata));
        let opts = if zero { DmaAllocationOptions::ZERO } else { DmaAllocationOptions::NONE };

        allocate_device_addressable_memory(Bytes(size), opts)
            .map(|(phys, virt)| Self { phys, virt: NonNull::from_raw_parts(virt.cast(), metadata) })
    }

    pub fn physical_address(&self) -> PhysicalAddress {
        self.phys
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.virt.as_ptr() }
    }
}

impl<T> DmaRegion<MaybeUninit<T>> {
    pub unsafe fn new() -> Result<Self, SyscallError>
    where
        T: Pointee<Metadata = ()>,
    {
        let (phys, virt) =
            allocate_device_addressable_memory(Bytes(core::mem::size_of::<T>()), DmaAllocationOptions::NONE)?;
        Result::Ok(Self { phys, virt: NonNull::from_raw_parts(virt.cast(), ()) })
    }

    pub unsafe fn zeroed() -> Result<Self, SyscallError>
    where
        T: Pointee<Metadata = ()>,
    {
        let (phys, virt) =
            allocate_device_addressable_memory(Bytes(core::mem::size_of::<T>()), DmaAllocationOptions::ZERO)?;
        Result::Ok(Self { phys, virt: NonNull::from_raw_parts(virt.cast(), ()) })
    }

    pub unsafe fn assume_init(self) -> DmaRegion<T> {
        let phys = self.phys;
        let virt = self.virt;
        core::mem::forget(self);

        DmaRegion { phys, virt: virt.cast() }
    }
}

// TODO: figure out if this is sound lol
impl<T: ?Sized> core::ops::Deref for DmaRegion<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.virt.as_ptr() }
    }
}

impl<T: ?Sized> core::ops::DerefMut for DmaRegion<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.virt.as_ptr() }
    }
}

impl<T: ?Sized> core::ops::Drop for DmaRegion<T> {
    // TODO: dealloc memory
    fn drop(&mut self) {}
}

pub struct DmaElement<'a, T> {
    phys: PhysicalAddress,
    virt: NonNull<T>,
    _region: &'a DmaRegion<[T]>,
}

impl<'a, T> DmaElement<'a, T> {
    pub fn physical_address(&self) -> PhysicalAddress {
        self.phys
    }

    pub fn get(&self) -> NonNull<T> {
        self.virt
    }
}
