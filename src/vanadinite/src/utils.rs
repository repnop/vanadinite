// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[repr(transparent)]
pub struct LinkerSymbol(u8);

impl LinkerSymbol {
    pub fn as_ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }

    pub fn as_mut_ptr(&'static mut self) -> *mut u8 {
        self as *mut Self as *mut u8
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

    pub const unsafe fn get(&self) -> *mut T {
        self.0.get()
    }
}

unsafe impl<T> Sync for StaticMut<T> {}
unsafe impl<T> Send for StaticMut<T> {}

#[allow(dead_code)]
#[inline(always)]
pub fn manual_debug_point() {
    unsafe {
        asm!("1: j 1b");
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

impl_units!(u16, u32, u64, u128, i16, i32, i64, i128);

pub mod volatile {
    use core::cell::UnsafeCell;
    #[derive(Debug, Clone, Copy)]
    pub struct Read;
    #[derive(Debug, Clone, Copy)]
    pub struct Write;
    #[derive(Debug, Clone, Copy)]
    pub struct ReadWrite;

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct Volatile<T, Direction = ReadWrite>(UnsafeCell<T>, core::marker::PhantomData<Direction>);

    impl<T: Copy + 'static> Volatile<T, Read> {
        pub fn read(&self) -> T {
            unsafe { self.0.get().read_volatile() }
        }
    }

    impl<T: Copy + 'static> Volatile<T, Write> {
        pub fn write(&self, val: T) {
            unsafe { self.0.get().write_volatile(val) }
        }
    }

    impl<T: Copy + 'static> Volatile<T, ReadWrite> {
        pub fn read(&self) -> T {
            unsafe { self.0.get().read_volatile() }
        }

        pub fn write(&self, val: T) {
            unsafe { self.0.get().write_volatile(val) }
        }
    }

    impl<T: Copy, const N: usize> core::ops::Index<usize> for Volatile<[T; N], Read> {
        type Output = Volatile<T>;

        fn index(&self, index: usize) -> &Self::Output {
            unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
        }
    }

    impl<T: Copy, const N: usize> core::ops::Index<usize> for Volatile<[T; N], ReadWrite> {
        type Output = Volatile<T>;

        fn index(&self, index: usize) -> &Self::Output {
            unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
        }
    }
}
