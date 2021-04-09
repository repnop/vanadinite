// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::sync::Mutex;
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Lazy<T, F = fn() -> T> {
    done_init: AtomicBool,
    init_mutex: Mutex<()>,
    value: UnsafeCell<MaybeUninit<T>>,
    f: F,
}

impl<T, F: Fn() -> T> Lazy<T, F> {
    pub const fn new(f: F) -> Self {
        Self {
            done_init: AtomicBool::new(false),
            init_mutex: Mutex::new(()),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            f,
        }
    }

    fn init_if_needed(&self) -> &T {
        match self.done_init.load(Ordering::Acquire) {
            true => unsafe { (&*self.value.get()).assume_init_ref() },
            false => {
                let _lock = self.init_mutex.lock();

                match self.done_init.load(Ordering::Acquire) {
                    // Someone else just init'd it
                    true => unsafe { (&*self.value.get()).assume_init_ref() },
                    false => unsafe {
                        self.value.get().write(MaybeUninit::new((self.f)()));
                        self.done_init.store(true, Ordering::Release);
                        (&*self.value.get()).assume_init_ref()
                    },
                }
            }
        }
    }
}

impl<T, F: Fn() -> T> core::ops::Deref for Lazy<T, F> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.init_if_needed()
    }
}

unsafe impl<T, F: Fn() -> T> Send for Lazy<T, F> {}
unsafe impl<T, F: Fn() -> T> Sync for Lazy<T, F> {}
