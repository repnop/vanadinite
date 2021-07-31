// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[macro_export]
macro_rules! dbg {
    ($e:expr) => {{
        let value = $e;
        crate::println!(concat!(stringify!($e), " = {:?}"), value);
        value
    }};
}

extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    pub fn as_ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }

    pub fn as_mut_ptr(&'static self) -> *mut u8 {
        self as *const Self as *mut Self as *mut u8
    }

    pub fn as_usize(&'static self) -> usize {
        self.as_ptr() as usize
    }
}

unsafe impl Sync for LinkerSymbol {}
unsafe impl Send for LinkerSymbol {}

#[repr(transparent)]
pub struct StaticMut<T>(core::cell::UnsafeCell<T>);

impl<T> StaticMut<T> {
    pub const fn new(t: T) -> Self {
        Self(core::cell::UnsafeCell::new(t))
    }

    /// # Safety
    ///
    /// You must ensure that no data races nor aliasing occurs when using the
    /// resulting pointer
    pub const fn get(&self) -> *mut T {
        self.0.get()
    }
}

unsafe impl<T> Sync for StaticMut<T> {}
unsafe impl<T> Send for StaticMut<T> {}

pub fn micros(ticks: u64, hz: u64) -> u64 {
    // ticks / hz -> second
    // ticks / (hz / 1000) -> millisecond
    // ticks / (hz / 1000 / 1000) -> microsecond
    ticks / (hz / 1000 / 1000)
}

pub fn time_parts(micros: u64) -> (u64, u64, u64) {
    let seconds = micros / (1000 * 1000);
    let micros_left = micros % (1000 * 1000);
    let millis = micros_left / 1000;
    let micros = micros_left % 1000;
    (seconds, millis, micros)
}

pub fn ticks_per_us(target_us: u64, hz: u64) -> u64 {
    (hz / 1000 / 1000) * target_us
}

#[allow(dead_code)]
#[inline(always)]
pub fn manual_debug_point() {
    unsafe {
        loop {
            asm!("nop");
        }
    }
}

pub fn round_up_to_next(n: usize, size: usize) -> usize {
    assert!(size.is_power_of_two());

    if n % size == 0 {
        n
    } else {
        (n & !(size - 1)) + size
    }
}

pub trait Units: core::ops::Mul<Self, Output = Self> + Sized {
    const KIB: Self;

    fn kib(self) -> Self {
        self * <Self as Units>::KIB
    }

    fn mib(self) -> Self {
        self * Self::KIB * Self::KIB
    }

    fn gib(self) -> Self {
        self * Self::KIB * Self::KIB * Self::KIB
    }
}

macro_rules! impl_units {
    ($($t:ty),+) => {
        $(
            impl Units for $t {
                const KIB: Self = 1024;
            }
        )+
    };
}

impl_units!(u16, u32, u64, u128, i16, i32, i64, i128, usize, isize);
