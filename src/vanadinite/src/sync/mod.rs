// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod lazy;
mod mutex;
mod rwlock;

pub use lazy::Lazy;
pub use lock_api::{self, RawMutex};
pub use rwlock::SpinRwLock;

#[derive(Debug)]
#[repr(transparent)]
pub struct RwLock<T>(lock_api::RwLock<rwlock::SpinRwLock, T>);

impl<T> RwLock<T> {
    pub const fn new(value: T) -> Self {
        Self(lock_api::RwLock::const_new(rwlock::SpinRwLock::new(), value))
    }
}

impl<T> core::ops::Deref for RwLock<T> {
    type Target = lock_api::RwLock<rwlock::SpinRwLock, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for RwLock<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Mutex<T>(lock_api::Mutex<mutex::SpinMutex, T>);

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self(lock_api::Mutex::const_new(mutex::SpinMutex::new(), value))
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T> core::ops::Deref for Mutex<T> {
    type Target = lock_api::Mutex<mutex::SpinMutex, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Mutex<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
