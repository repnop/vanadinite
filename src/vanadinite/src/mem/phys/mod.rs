// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod bitmap;

use crate::{mem::paging::PhysicalAddress, sync::Mutex};
use bitmap::BitmapAllocator;

#[cfg(any(not(any(feature = "pmalloc.allocator.buddy")), feature = "pmalloc.allocator.bitmap"))]
pub static PHYSICAL_MEMORY_ALLOCATOR: Mutex<BitmapAllocator> = Mutex::new(BitmapAllocator::new());

pub unsafe trait PhysicalMemoryAllocator {
    fn init(&mut self, start: *mut u8, end: *mut u8);
    unsafe fn alloc(&mut self) -> Option<PhysicalPage>;
    unsafe fn alloc_contiguous(&mut self, n: usize) -> Option<PhysicalPage>;
    unsafe fn dealloc(&mut self, page: PhysicalPage);
    unsafe fn dealloc_contiguous(&mut self, page: PhysicalPage, n: usize);
    unsafe fn set_used(&mut self, page: PhysicalPage);
    unsafe fn set_unused(&mut self, page: PhysicalPage);
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PhysicalPage(*mut u8);

impl PhysicalPage {
    pub fn from_ptr(ptr: *mut u8) -> Self {
        assert_eq!(ptr as usize % 4096, 0, "unaligned physical page creation");
        Self(ptr)
    }

    pub fn as_phys_address(self) -> PhysicalAddress {
        PhysicalAddress::from_ptr(self.0)
    }
}

pub fn alloc_page() -> PhysicalPage {
    unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("out of memory") }
}

pub fn zalloc_page() -> PhysicalPage {
    let page = alloc_page();
    let ptr = crate::kernel_patching::phys2virt(page.as_phys_address()).as_mut_ptr().cast::<u64>();

    unsafe {
        for i in 0..(4096 / 8) {
            *ptr.add(i) = 0;
        }
    }

    page
}
