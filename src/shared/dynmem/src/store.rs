// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2026 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    alloc::{AllocError, Layout},
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
};

/// A memory allocator which provides handles to allocated blocks of memory that
/// resolve to pointers using the [`Store::resolve`] method.
///
/// ### Safety
///
/// Allocators which implement this trait must abide by the safety guarantees
/// and contracts speified on this trait implementation.
pub unsafe trait Store {
    /// The handle type which is provided by [`Store::allocate`]. This type is
    /// then used to resolve a pointer to memory allocation which is valid until
    /// any of the following conditions are met:
    ///
    /// 1. The type wrapping the [`Store`] or the [`Store`] itself is moved.
    /// 2. A new memory allocation is made via [`Store::allocate`].
    /// 3. A previous allocation is deallocated with [`Store::deallocate`].
    type Handle: Copy;

    fn dangling_layout(layout: Layout) -> Result<Self::Handle, AllocError>;
    fn dangling<T>() -> Result<Self::Handle, AllocError> {
        Self::dangling_layout(Layout::new::<T>())
    }

    /// Attempt to allocate a new block of memory for the given layout, which
    /// may be larger than the requested size but is at least aligned to the
    /// same alignment that is requested. Returns a [`Store::Handle`] which can
    /// then be passed to [`Store::resolve`] to retrieve a pointer to the
    /// allocated block of memory.
    ///
    /// This method may invalidate existing [`Store::resolve`]d pointers.
    fn allocate(&self, layout: Layout) -> Result<Self::Handle, AllocError>;

    /// Deallocate a previously allocated block of memory via ihe
    /// [`Store::Handle`] returned after allocation.
    ///
    /// This method may invalidate existing [`Store::resolve`]d pointers.
    ///
    /// ### Safety
    ///
    /// The handle passed to this method must:
    ///
    /// 1. Have been allocated by the same [`Store`] that this method is invoked
    ///    on.
    /// 2. Have the same [`Layout`] as was provided to the [`Store::allocate`]
    ///    method.
    unsafe fn deallocate(&self, handle: Self::Handle, layout: Layout);

    /// Resolves a previously allocated [`Store::Handle`] to a pointer
    /// representing the available memory allocated for that object.
    ///
    /// This method MUST NOT invalidate existing pointers previously returned by
    /// other [`Store::resolve`] calls.
    ///
    /// ### Safety
    ///
    /// The handle passed to this method must have been allocated by the
    /// [`Store`] that this method is invoked on.
    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<[u8]>;

    /// Like [`Store::allocate`] but also zeroes the allocated block of memory
    /// before returning the [`Store::Handle`].
    ///
    /// This method may invalidate existing [`Store::resolve`]d pointers.
    #[inline]
    fn allocate_zeroed(&self, layout: Layout) -> Result<Self::Handle, AllocError> {
        match layout.size() {
            0 => Self::dangling_layout(layout),
            _ => {
                let handle = self.allocate(layout)?;

                if layout.size() > 0 {
                    let ptr = unsafe { self.resolve(handle) };
                    unsafe { ptr.cast::<u8>().write_bytes(0, ptr.len()) };
                }

                Ok(handle)
            }
        }
    }

    /// Attempt to resize a previously allocated block of memory represented by `handle`.
    ///
    /// This method may invalidate existing [`Store::resolve`]d pointers.
    ///
    /// ### Safety
    ///
    /// See [`Store::deallocate`].
    #[inline]
    unsafe fn resize(
        &self,
        handle: Self::Handle,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<Self::Handle, AllocError> {
        // see if we can reuse the existing allocation instead of always forcing a new one;
        if old_layout.align() >= new_layout.align() && old_layout.size() >= new_layout.size() {
            return Ok(handle);
        } else if new_layout.size() == 0 {
            unsafe { self.deallocate(handle, old_layout) };
            return Self::dangling_layout(new_layout);
        }

        let new_handle = self.allocate(new_layout)?;
        let new_ptr = unsafe { self.resolve(new_handle) };
        let old_ptr = unsafe { self.resolve(handle) };

        let byte_ptr = new_ptr.cast::<u8>();
        unsafe { byte_ptr.copy_from_nonoverlapping(old_ptr.cast::<u8>(), old_ptr.len().min(new_ptr.len())) };
        unsafe { self.deallocate(handle, old_layout) };

        Ok(new_handle)
    }

    /// Like [`Store::resize`] but zeroes the allocation after resizing.
    ///
    /// ### Safety
    ///
    /// See [`Store::deallocate`].
    #[inline]
    unsafe fn resize_zeroed(
        &self,
        handle: Self::Handle,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<Self::Handle, AllocError> {
        match new_layout.size() {
            0 => {
                unsafe { self.deallocate(handle, old_layout) };
                Self::dangling_layout(new_layout)
            }
            _ => {
                let new_handle = unsafe { self.resize(handle, old_layout, new_layout)? };
                let ptr = unsafe { self.resolve(new_handle) };
                unsafe { ptr.as_uninit_slice_mut().fill(MaybeUninit::new(0)) };

                Ok(new_handle)
            }
        }
    }
}

/// A [`Store`] which does not invalidate existing pointers when allocating or
/// deallocating other blocks of memory.
///
/// ### Safety
///
/// A [`Store`] which implements this trait MUST uphold the guarantees provided
/// in the description.
pub unsafe trait StablePtrStore: Store {}

/// A [`StablePtrStore`] which does not invalidate existing pointers when the
/// store is moved.
///
/// ### Safety
///
/// A [`Store`] which implements this trait MUST uphold the guarantees provided
/// in the description.
pub unsafe trait PinnedPtrStore: StablePtrStore {}

/// A stack allocated [`Store`] that has size and alignment based on provided
/// generic `T` to use as a backing for a single allocation at a time.
pub struct InlineSingleStore<T> {
    backing: UnsafeCell<MaybeUninit<T>>,
    allocated: Cell<bool>,
}

impl<T> InlineSingleStore<T> {
    pub const fn new() -> Self {
        Self { backing: UnsafeCell::new(MaybeUninit::uninit()), allocated: Cell::new(false) }
    }
}

impl<T> Default for InlineSingleStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<T> StablePtrStore for InlineSingleStore<T> {}
unsafe impl<T> Store for InlineSingleStore<T> {
    type Handle = ();

    fn dangling_layout(layout: Layout) -> Result<Self::Handle, AllocError> {
        if layout.align() > core::mem::align_of::<T>() || layout.size() > core::mem::size_of::<T>() {
            return Err(AllocError);
        }

        Ok(())
    }

    fn allocate(&self, layout: Layout) -> Result<Self::Handle, AllocError> {
        let invalid_layout = layout.align() > core::mem::align_of::<T>() || layout.size() > core::mem::size_of::<T>();
        let allocated = self.allocated.get();
        if invalid_layout || allocated {
            return Err(AllocError);
        }

        self.allocated.set(true);

        Ok(())
    }

    unsafe fn deallocate(&self, _: Self::Handle, _: Layout) {
        self.allocated.set(false);
    }

    unsafe fn resolve(&self, _: Self::Handle) -> NonNull<[u8]> {
        NonNull::from_raw_parts(
            unsafe { NonNull::new(self.backing.get().cast::<u8>()).unwrap_unchecked() },
            core::mem::size_of::<T>(),
        )
    }
}

#[doc(hidden)]
pub const fn typed_array_store_bitmap_len<const N: usize>() -> usize {
    if N <= 64 { 1 } else { N.next_multiple_of(64) / 64 }
}

pub struct TypedArrayStore<T, const N: usize>
where
    [(); typed_array_store_bitmap_len::<N>()]:,
{
    array: [UnsafeCell<MaybeUninit<T>>; N],
    bitmap: [Cell<u64>; typed_array_store_bitmap_len::<N>()],
}

impl<T, const N: usize> TypedArrayStore<T, N>
where
    [(); typed_array_store_bitmap_len::<N>()]:,
{
    pub const CUTOFF: u32 = const { (N % 64) as u32 };

    pub const fn new() -> Self {
        Self {
            array: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            bitmap: [const { Cell::new(0) }; typed_array_store_bitmap_len::<N>()],
        }
    }
}

impl<T, const N: usize> Default for TypedArrayStore<T, N>
where
    [(); typed_array_store_bitmap_len::<N>()]:,
{
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<T, const N: usize> StablePtrStore for TypedArrayStore<T, N> where [(); typed_array_store_bitmap_len::<N>()]: {}
unsafe impl<T, const N: usize> Store for TypedArrayStore<T, N>
where
    [(); typed_array_store_bitmap_len::<N>()]:,
{
    type Handle = TypedArrayAllocHandle;

    fn dangling_layout(layout: Layout) -> Result<Self::Handle, AllocError> {
        Ok(TypedArrayAllocHandle(layout.align() & (1 << (usize::BITS - 1))))
    }

    fn allocate(&self, layout: Layout) -> Result<Self::Handle, AllocError> {
        if layout.align() > core::mem::align_of::<T>() || layout.size() > core::mem::size_of::<T>() {
            return Err(AllocError);
        }

        match self.bitmap.iter().position(|n| n.get() != u64::MAX) {
            Some(index) => {
                let value = self.bitmap[index].get();
                let leading_ones = value.trailing_ones();
                // check if there were extra bits that don't actually have backing
                if index == self.bitmap.len() - 1 && leading_ones >= Self::CUTOFF {
                    return Err(AllocError);
                }

                self.bitmap[index].set(value | (1 << leading_ones));
                Ok(TypedArrayAllocHandle(index * 64 + leading_ones as usize))
            }
            None => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, handle: Self::Handle, _: Layout) {
        let (index, bit) = (handle.0 / 64, handle.0 % 64);
        self.bitmap[index].update(|n| n & !(1 << bit));
    }

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<[u8]> {
        NonNull::from_raw_parts(
            unsafe { NonNull::new_unchecked(self.array.get_unchecked(handle.0).get()) },
            core::mem::size_of::<T>(),
        )
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct TypedArrayAllocHandle(usize);

#[macro_export]
macro_rules! pin_store {
    ($store:ident) => {
        let $store = $store;
        let $store = unsafe { $crate::store::PinnedStore::new_unchecked(&$store) };
    };
}

/// A convenience [`StablePtrStore`] wrapper that pins the provided [`Store`] so
/// that it can implement [`PinnedPtrStore`].
#[repr(transparent)]
pub struct PinnedStore<'store, S: Store>(Pin<&'store S>);

impl<'store, S: Store> PinnedStore<'store, S> {
    /// Creates a new [`PinnedStore`] from a reference to a [`Store`] which
    /// implements [`Unpin`].
    pub fn new(store: &'store S) -> Self
    where
        S: Unpin,
    {
        Self(Pin::new(store))
    }

    /// Creates a new [`PinnedStore`] from a reference to a [`Store`] which does
    /// not implement [`Unpin`] and so can not be safely created.
    ///
    /// ### Safety
    ///
    /// See [`Pin::new_unchecked`]'s docs for more info on the requirements for
    /// calling this function to be sound.
    pub unsafe fn new_unchecked(store: &'store S) -> Self {
        Self(unsafe { Pin::new_unchecked(store) })
    }
}

unsafe impl<'store, S: StablePtrStore> Store for PinnedStore<'store, S> {
    type Handle = S::Handle;

    fn allocate(&self, layout: Layout) -> Result<Self::Handle, AllocError> {
        self.0.allocate(layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<Self::Handle, AllocError> {
        self.0.allocate_zeroed(layout)
    }

    fn dangling<T>() -> Result<Self::Handle, AllocError> {
        S::dangling::<T>()
    }

    fn dangling_layout(layout: Layout) -> Result<Self::Handle, AllocError> {
        S::dangling_layout(layout)
    }

    unsafe fn deallocate(&self, handle: Self::Handle, layout: Layout) {
        unsafe { self.0.deallocate(handle, layout) };
    }

    unsafe fn resize(
        &self,
        handle: Self::Handle,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<Self::Handle, AllocError> {
        unsafe { self.0.resize(handle, old_layout, new_layout) }
    }

    unsafe fn resize_zeroed(
        &self,
        handle: Self::Handle,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<Self::Handle, AllocError> {
        unsafe { self.0.resize_zeroed(handle, old_layout, new_layout) }
    }

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<[u8]> {
        unsafe { self.0.resolve(handle) }
    }
}

unsafe impl<'store, S: StablePtrStore> StablePtrStore for PinnedStore<'store, S> {}
unsafe impl<'store, S: StablePtrStore> PinnedPtrStore for PinnedStore<'store, S> {}

#[cfg(test)]
mod test {
    extern crate std;

    use super::*;

    #[test]
    fn typed_array_store_works() {
        let store: TypedArrayStore<u32, 20> = TypedArrayStore::new();
        for i in 0..20 {
            assert!(store.allocate(Layout::new::<u32>()).is_ok(), "{i}");
        }

        assert!(store.allocate(Layout::new::<u32>()).is_err());
    }

    #[test]
    fn pinned_store_good() {
        let array_store: TypedArrayStore<u32, 8> = TypedArrayStore::new();
        pin_store!(array_store);
        let boxed = crate::boxed::Box::pin(4u32, array_store).unwrap();
        assert_eq!(*boxed, 4u32);
    }
}
