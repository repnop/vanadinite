// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct Lazy<T, F = fn() -> T> {
    init_state: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
    f: F,
}

impl<T, F: Fn() -> T> Lazy<T, F> {
    pub const fn new(f: F) -> Self {
        Self { init_state: AtomicUsize::new(0), value: UnsafeCell::new(MaybeUninit::uninit()), f }
    }

    fn init_if_needed(&self) -> &T {
        match self.init_state.load(Ordering::Acquire) {
            0b00 => match self.init_state.compare_exchange(0b00, 0b01, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => {
                    unsafe { self.value.get().write(MaybeUninit::new((self.f)())) };
                    self.init_state.store(0b10, Ordering::Release);
                    unsafe { self.get_ref() }
                }
                Err(_) => {
                    self.wait_for_init();
                    unsafe { self.get_ref() }
                }
            },
            0b01 => {
                self.wait_for_init();
                unsafe { self.get_ref() }
            }
            0b10 => unsafe { self.get_ref() },
            _ => unreachable!(),
        }
    }

    fn wait_for_init(&self) {
        while self.init_state.load(Ordering::Acquire) != 0b10 {
            // TODO: maybe add ability to specify instruction for stalling?
            // crate::asm::pause();
        }
    }

    unsafe fn get_ref(&self) -> &T {
        (*self.value.get()).assume_init_ref()
    }

    pub fn get_mut(&mut self) -> &mut T {
        // If we have exclusive access to this, we only need to check to see if
        // its been init'd yet
        match self.init_state.load(Ordering::Relaxed) {
            0b00 => {
                unsafe { self.value.get().write(MaybeUninit::new((self.f)())) };
                self.init_state.store(0b10, Ordering::Release);
                unsafe { (*self.value.get()).assume_init_mut() }
            }
            0b10 => unsafe { (*self.value.get()).assume_init_mut() },
            _ => unreachable!("this state should never be reached with exclusive access"),
        }
    }
}

impl<T, F: Fn() -> T> core::ops::Deref for Lazy<T, F> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.init_if_needed()
    }
}

impl<T, F: Fn() -> T> core::ops::DerefMut for Lazy<T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

unsafe impl<T: Send, F: Fn() -> T> Send for Lazy<T, F> {}
unsafe impl<T: Send, F: Fn() -> T> Sync for Lazy<T, F> {}
