// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod lazy;
pub mod mutex;
pub mod rwlock;

use core::sync::atomic::{AtomicPtr, Ordering};
pub use lazy::Lazy;
pub use mutex::SpinMutex;
pub use rwlock::SpinRwLock;

#[repr(transparent)]
pub struct AtomicConstPtr<T>(AtomicPtr<T>);

impl<T> AtomicConstPtr<T> {
    pub const fn new(ptr: *const T) -> Self {
        Self(AtomicPtr::new(ptr as *mut _))
    }

    pub fn store(&self, ptr: *const T, ordering: Ordering) {
        self.0.store(ptr as *mut _, ordering)
    }

    pub fn load(&self, ordering: Ordering) -> *const T {
        self.0.load(ordering)
    }
}

pub trait DeadlockDetection {
    fn would_deadlock(&self) -> bool;
    fn gather_metadata(&self);
    fn unlocked(&self);
}

#[derive(Debug, Clone, Copy)]
pub struct NoCheck;

impl const Default for NoCheck {
    fn default() -> Self {
        Self
    }
}

impl DeadlockDetection for NoCheck {
    fn would_deadlock(&self) -> bool {
        false
    }

    fn gather_metadata(&self) {}
    fn unlocked(&self) {}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Immediate;

impl DeadlockDetection for Immediate {
    fn would_deadlock(&self) -> bool {
        true
    }

    fn gather_metadata(&self) {}
    fn unlocked(&self) {}
}
