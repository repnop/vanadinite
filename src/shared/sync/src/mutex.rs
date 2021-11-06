// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct SpinMutex<T: Send> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T: Send> SpinMutex<T> {
    pub const fn new(data: T) -> Self {
        Self { lock: AtomicBool::new(false), data: UnsafeCell::new(data) }
    }

    pub fn with_lock<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        self.acquire_lock();
        let ret = f(unsafe { &mut *self.data.get() });
        self.unlock();

        ret
    }

    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        self.acquire_lock();
        SpinMutexGuard { lock: self }
    }

    fn acquire_lock(&self) {
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // TODO: maybe add ability to specify instruction for stalling?
            // crate::asm::pause();
        }
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

impl<T: Send> core::fmt::Debug for SpinMutex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpinMutex").finish_non_exhaustive()
    }
}

pub struct SpinMutexGuard<'a, T: Send> {
    lock: &'a SpinMutex<T>,
}

impl<T: Send> core::ops::Deref for SpinMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: Send> core::ops::DerefMut for SpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: Send> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.unlock()
    }
}
