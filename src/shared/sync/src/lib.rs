// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(const_fn_trait_bound)]
#![no_std]

mod lazy;
mod mutex;
mod rwlock;

use core::{
    marker::PhantomData,
    sync::atomic::{AtomicPtr, Ordering},
};
pub use lazy::Lazy;
pub use mutex::SpinMutex;
pub use rwlock::SpinRwLock;

#[repr(transparent)]
pub struct AtomicConstPtr<T>(AtomicPtr<T>, PhantomData<T>);

impl<T> AtomicConstPtr<T> {
    pub const fn new(ptr: *const T) -> Self {
        Self(AtomicPtr::new(ptr as *mut _), PhantomData)
    }

    pub fn store(&self, ptr: *const T, ordering: Ordering) {
        self.0.store(ptr as *mut _, ordering)
    }

    pub fn load(&self, ordering: Ordering) -> *const T {
        self.0.load(ordering)
    }
}
