// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct SpinRwLock<T: Send> {
    lock: AtomicUsize,
    readers: AtomicUsize,
    data: UnsafeCell<T>,
}

impl<T: Send> SpinRwLock<T> {
    pub const fn new(data: T) -> Self {
        Self { lock: AtomicUsize::new(0), readers: AtomicUsize::new(0), data: UnsafeCell::new(data) }
    }

    pub fn read(&self) -> ReadGuard<'_, T> {
        self.lock_shared();
        ReadGuard { lock: self }
    }

    pub fn write(&self) -> WriteGuard<'_, T> {
        self.lock_exclusive();
        WriteGuard { lock: self }
    }

    fn lock_shared(&self) {
        while !self.try_lock_shared() {
            crate::asm::pause();
        }
    }

    fn try_lock_shared(&self) -> bool {
        match self.lock.compare_exchange_weak(0, 0b01, Ordering::AcqRel, Ordering::Relaxed) {
            Ok(_) => {
                self.readers.fetch_add(1, Ordering::AcqRel);
                true
            }
            Err(_) => match self.lock.load(Ordering::Acquire) {
                0b01 => {
                    self.readers.fetch_add(1, Ordering::AcqRel);
                    true
                }
                _ => false,
            },
        }
    }

    fn unlock_shared(&self) {
        if self.readers.fetch_sub(1, Ordering::Acquire) == 1 {
            self.lock.store(0b100, Ordering::Release);
            match self.readers.load(Ordering::Acquire) {
                0 => self.lock.store(0, Ordering::Release),
                _ => self.lock.store(0b01, Ordering::Release),
            }
        }
    }

    fn lock_exclusive(&self) {
        while !self.try_lock_exclusive() {
            crate::asm::pause();
        }
    }

    fn try_lock_exclusive(&self) -> bool {
        self.lock.compare_exchange_weak(0, 0b10, Ordering::AcqRel, Ordering::Relaxed).is_ok()
    }

    fn unlock_exclusive(&self) {
        self.lock.store(0, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for SpinRwLock<T> {}
unsafe impl<T: Send> Sync for SpinRwLock<T> {}

pub struct WriteGuard<'a, T: Send> {
    lock: &'a SpinRwLock<T>,
}

impl<T: Send> core::ops::Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: Send> core::ops::DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: Send> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.unlock_exclusive();
    }
}

pub struct ReadGuard<'a, T: Send> {
    lock: &'a SpinRwLock<T>,
}

impl<'a, T: Send> ReadGuard<'a, T> {
    pub fn upgrade(self) -> WriteGuard<'a, T> {
        // Copy reference to lock, then don't run `Drop` for self
        let lock = self.lock;
        core::mem::forget(self);

        lock.unlock_shared();
        lock.write()
    }
}

impl<T: Send> core::ops::Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: Send> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.unlock_shared();
    }
}
