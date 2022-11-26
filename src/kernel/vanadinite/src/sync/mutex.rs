// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::TIMER_FREQ;

use super::{DeadlockDetection, NoCheck};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct SpinMutex<T: Send, D: DeadlockDetection = NoCheck> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
    deadlock_detection: D,
}

impl<T: Send, D: DeadlockDetection + ~const Default> SpinMutex<T, D> {
    pub const fn new(data: T) -> Self {
        Self { lock: AtomicBool::new(false), data: UnsafeCell::new(data), deadlock_detection: D::default() }
    }
}

impl<T: Send, D: DeadlockDetection> SpinMutex<T, D> {
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
                self.deadlock_detection.gather_metadata();
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

    /// Acquire the lock and return pointers to the data and lock atomic.
    ///
    /// # Safety
    /// This function requires that the returned pointer is not access after the
    /// [`AtomicBool`] is set to `false`.
    pub unsafe fn raw_locked_parts(&self) -> (*mut T, *const AtomicBool) {
        self.acquire_lock();
        (self.data.get(), &self.lock)
    }

    #[track_caller]
    fn acquire_lock(&self) {
        let freq = TIMER_FREQ.load(Ordering::Relaxed);
        let max_wait_time = crate::utils::ticks_per_us(1 * 1000 * 1000, freq);
        let start_time = crate::csr::time::read();
        let end_time = start_time + max_wait_time;

        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            if self.deadlock_detection.would_deadlock() {
                panic!("Deadlock detected");
            } else if crate::csr::time::read() >= end_time {
                panic!("Likely deadlock detected -- reached maximum wait time");
            }

            core::hint::spin_loop();
        }

        self.deadlock_detection.gather_metadata();
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
