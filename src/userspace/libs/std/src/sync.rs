// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub use alloc::sync::*;
pub use core::sync::*;

/// A [`core::cell::RefCell`] that implements `Send` and `Sync` to be suitable
/// for use in `static`s.
#[derive(Debug)]
pub struct SyncRefCell<T: ?Sized>(core::cell::RefCell<T>);

impl<T> SyncRefCell<T> {
    pub const fn new(value: T) -> Self {
        Self(core::cell::RefCell::new(value))
    }
}

unsafe impl<T> Send for SyncRefCell<T> {}
unsafe impl<T> Sync for SyncRefCell<T> {}

impl<T> core::ops::Deref for SyncRefCell<T> {
    type Target = core::cell::RefCell<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for SyncRefCell<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A [`alloc::rc::Rc`] that implements `Send` and `Sync`.
#[derive(Debug)]
pub struct SyncRc<T: ?Sized>(alloc::rc::Rc<T>);

impl<T> SyncRc<T> {
    pub fn new(value: T) -> Self {
        Self(alloc::rc::Rc::new(value))
    }
}

unsafe impl<T> Send for SyncRc<T> {}
unsafe impl<T> Sync for SyncRc<T> {}

impl<T> core::ops::Deref for SyncRc<T> {
    type Target = alloc::rc::Rc<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for SyncRc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Clone for SyncRc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
