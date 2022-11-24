// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{DeadlockDetection, NoCheck};
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

#[repr(C)]
pub struct StableSpinMutex<T: Send> {
    lock: AtomicU64,
    data: UnsafeCell<T>,
}

impl<T: Send> StableSpinMutex<T> {
    pub fn new(value: T) -> Self {
        Self { lock: AtomicU64::new(0), data: UnsafeCell::new(value) }
    }

    pub unsafe fn lock_into_parts(&self) -> (*mut T, &AtomicU64) {
        self.acquire_lock();

        (self.data.get(), &self.lock)
    }

    fn acquire_lock(&self) {
        while self.lock.compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            crate::asm::pause();
        }
    }

    unsafe fn unlock(&self) {
        self.lock.store(0, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for StableSpinMutex<T> {}
unsafe impl<T: Send> Sync for StableSpinMutex<T> {}

pub struct SpinMutex<T: Send, D: DeadlockDetection = NoCheck> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
    deadlock_detection: PhantomData<D>,
    deadlock_metadata: AtomicUsize,
}

impl<T: Send, D: DeadlockDetection> SpinMutex<T, D> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
            deadlock_detection: PhantomData,
            deadlock_metadata: AtomicUsize::new(0),
        }
    }

    pub fn with_lock<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        self.acquire_lock();
        let ret = f(unsafe { &mut *self.data.get() });
        self.unlock();

        ret
    }

    #[track_caller]
    pub fn lock(&self) -> SpinMutexGuard<'_, T, D> {
        self.acquire_lock();
        SpinMutexGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T, D>> {
        match self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => {
                self.deadlock_metadata.store(D::gather_metadata(), Ordering::Release);
                Some(SpinMutexGuard { lock: self })
            }
            Err(_) => None,
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        // Safety: `&mut self` enforces that there's no shared references
        // lingering, so its safe to immediate get the underlying data
        unsafe { &mut *self.data.get() }
    }

    #[track_caller]
    fn acquire_lock(&self) {
        let mut spin_check_count = 100;

        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            if spin_check_count != 0 && D::would_deadlock(self.deadlock_metadata.load(Ordering::Acquire)) {
                panic!("Deadlock detected");
            }

            spin_check_count -= 1;
        }

        self.deadlock_metadata.store(D::gather_metadata(), Ordering::Release);
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}

unsafe impl<T: Send, D: DeadlockDetection> Send for SpinMutex<T, D> {}
unsafe impl<T: Send, D: DeadlockDetection> Sync for SpinMutex<T, D> {}

impl<T: Send, D: DeadlockDetection> core::fmt::Debug for SpinMutex<T, D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpinMutex").finish_non_exhaustive()
    }
}

pub struct SpinMutexGuard<'a, T: Send, D: DeadlockDetection> {
    lock: &'a SpinMutex<T, D>,
}

impl<T: Send, D: DeadlockDetection> core::ops::Deref for SpinMutexGuard<'_, T, D> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: Send, D: DeadlockDetection> core::ops::DerefMut for SpinMutexGuard<'_, T, D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: Send, D: DeadlockDetection> Drop for SpinMutexGuard<'_, T, D> {
    fn drop(&mut self) {
        self.lock.unlock()
    }
}