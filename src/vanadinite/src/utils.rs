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

#[allow(dead_code)]
#[inline(always)]
pub fn manual_debug_point() {
    unsafe {
        asm!("1: j 1b");
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Volatile<T>(T);

impl<T: Copy + 'static> Volatile<T> {
    pub fn read(&self) -> T {
        unsafe { (self as *const _ as *const T).read_volatile() }
    }

    pub fn write(&mut self, val: T) {
        unsafe { (self as *mut _ as *mut T).write_volatile(val) }
    }
}

impl<T: Copy, const N: usize> core::ops::Index<usize> for Volatile<[T; N]> {
    type Output = Volatile<T>;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
    }
}

impl<T: Copy, const N: usize> core::ops::IndexMut<usize> for Volatile<[T; N]> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut core::mem::transmute::<_, &mut [Volatile<T>; N]>(self)[index] }
    }
}
