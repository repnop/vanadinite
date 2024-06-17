// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2026 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::store::{PinnedPtrStore, Store};
use core::{
    alloc::{AllocError, Layout},
    marker::Unsize,
    pin::Pin,
    ptr::Pointee,
};

/// A [`PinnedStore`] allocated `T`.
pub struct Box<T: ?Sized, S: Store> {
    handle: S::Handle,
    store: S,
    metadata: <T as Pointee>::Metadata,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Sized, S: Store> Box<T, S> {
    /// Allocates storage from the provided [`Store`] and places `value`
    /// into it. Does not allocate if `T` is zero-sized.
    ///
    /// ##### Example
    ///
    /// ```
    /// # use dynmem::{boxed::Box, store::InlineSingleStore};
    /// let my_boxed_i32 = Box::new(42i32, InlineSingleStore::<i32>::new());
    /// ```
    pub fn new(value: T, store: S) -> Result<Self, AllocError> {
        let handle = match core::mem::size_of::<T>() {
            0 => S::dangling::<T>()?,
            _ => {
                let handle = store.allocate(Layout::new::<T>())?;
                let ptr = unsafe { store.resolve(handle) };
                unsafe { ptr.cast::<T>().write(value) };
                handle
            }
        };

        Ok(Self { handle, store, metadata: (), _marker: core::marker::PhantomData })
    }

    /// Return a pointer to the allocated `T`.
    pub fn as_handle(this: &Self) -> S::Handle {
        this.handle
    }

    /// Reconstruct a [`Box<T, S>`] from its component parts.
    ///
    /// # Safety
    ///
    /// The handle must have been allocated by the same store that is passed to
    /// this function, otherwise the behavior is undefined whenever the [`Box<T,
    /// S>`] is resolved or dropped.
    pub const unsafe fn from_raw_parts(handle: S::Handle, store: S) -> Self {
        Self { handle, store, metadata: (), _marker: core::marker::PhantomData }
    }

    pub fn unsize<U: ?Sized>(this: Self) -> Box<U, S>
    where
        T: Unsize<U>,
    {
        let metadata = core::ptr::metadata(core::ptr::null::<T>() as *const U);
        let handle = this.handle;
        let store = unsafe { (&raw const this.store).read() };

        let new = Box { handle, store, metadata, _marker: core::marker::PhantomData };
        core::mem::forget(this);
        new
    }

    /// Extracts the boxed value and returns it, dropping the [`Store`] in the process.
    ///
    /// ```
    /// # use dynmem::{boxed::Box, store::InlineSingleStore};
    /// let my_boxed_value = Box::new(42i32, InlineSingleStore::<i32>::new()).unwrap();
    /// let my_unboxed_value = Box::into_inner(my_boxed_value);
    ///
    /// assert_eq!(my_unboxed_value, 42);
    /// ```
    pub fn into_inner(this: Self) -> T {
        let mut this = core::mem::ManuallyDrop::new(this);
        let ptr = unsafe { this.store.resolve(this.handle).cast::<T>().as_ptr() };
        let layout = unsafe { Layout::for_value_raw(ptr) };
        let value = unsafe { core::ptr::read(ptr) };

        unsafe { this.store.deallocate(this.handle, layout) };
        unsafe { core::ptr::drop_in_place(&raw mut this.store) };

        value
    }
}

impl<T: ?Sized, S: Store> Box<T, S> {
    /// Return a reference to the [`PinnedStore`].
    pub const fn store(this: &Self) -> &S {
        &this.store
    }

    pub fn as_ptr(this: &Self) -> *mut T {
        let thin_ptr = unsafe { this.store.resolve(this.handle) };
        core::ptr::from_raw_parts_mut(thin_ptr.cast::<()>().as_ptr(), this.metadata)
    }
}

impl<T: ?Sized, S: PinnedPtrStore> Unpin for Box<T, S> {}
impl<T: Sized, S: PinnedPtrStore> Box<T, S> {
    /// Attempts to create a new `Pin<Box<T, S>>`. If `T` is `!Unpin`, the value
    /// is unable to be safely moved while it exists.
    pub fn pin(value: T, store: S) -> Result<Pin<Self>, AllocError> {
        let this = Self::new(value, store)?;
        unsafe { Ok(Pin::new_unchecked(this)) }
    }

    /// Deconstruct this [`Box<T, S>`] into a pointer to the allocated value and
    /// the [`PinnedStore`] used to allocate it.
    pub fn into_raw_parts(this: Self) -> (*mut T, S) {
        let ptr = Self::as_ptr(&this);
        let store = unsafe { (&raw const this.store).read() };
        core::mem::forget(this);

        (ptr, store)
    }
}

impl<T: ?Sized, S: Store> core::ops::Deref for Box<T, S> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*Self::as_ptr(self) }
    }
}

impl<T: ?Sized, S: Store> core::ops::DerefMut for Box<T, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *Self::as_ptr(self) }
    }
}

unsafe impl<#[may_dangle] T: ?Sized, S: Store> core::ops::Drop for Box<T, S> {
    fn drop(&mut self) {
        let ptr = Self::as_ptr(self);
        let layout = unsafe { Layout::for_value_raw(ptr) };

        if layout.size() != 0 {
            unsafe { core::ptr::drop_in_place(ptr) };
            unsafe { self.store.deallocate(self.handle, layout) };
        }
    }
}

impl<T: ?Sized + core::fmt::Debug, S: Store> core::fmt::Debug for Box<T, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<T: Iterator + ?Sized, S: Store> Iterator for Box<T, S> {
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        (**self).next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InlineSingleStore;

    #[test]
    fn it_works() {
        let mut stack_box = Box::new(42i32, InlineSingleStore::<u32>::new()).unwrap();
        assert_eq!(*stack_box, 42);
        *stack_box += 1;
        assert_eq!(*stack_box, 43);
    }

    #[test]
    fn unsized_box() {
        let sized = Box::new(32i32, InlineSingleStore::<u32>::new()).unwrap();
        let unsized_box = Box::unsize::<dyn core::fmt::Debug>(sized);

        assert_eq!(std::format!("{unsized_box:?}"), std::format!("{:?}", 32i32));
    }
}
