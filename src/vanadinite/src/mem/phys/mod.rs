// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod bitmap;

use crate::{mem::paging::PhysicalAddress, sync::Mutex};
use bitmap::BitmapAllocator;

#[cfg(any(not(any(feature = "pmalloc.allocator.buddy")), feature = "pmalloc.allocator.bitmap"))]
pub static PHYSICAL_MEMORY_ALLOCATOR: Mutex<BitmapAllocator> = Mutex::new(BitmapAllocator::new());

pub unsafe trait PhysicalMemoryAllocator {
    /// # Safety
    ///
    /// `start` and `end` must form a valid region of physical memory that is
    /// accessible to the kernel
    unsafe fn init(&mut self, start: *mut u8, end: *mut u8);

    /// # Safety
    ///
    /// This method must not return the same physical page multiple times
    /// without it having been deallocated before being reused each time
    unsafe fn alloc(&mut self) -> Option<PhysicalPage>;

    /// # Safety
    ///
    /// The requirements for this method are the same as [`alloc`], but apply to
    /// the entire range returned
    unsafe fn alloc_contiguous(&mut self, n: usize) -> Option<PhysicalPage>;

    /// # Safety
    ///
    /// See the memory safety requirements of [`set_unused`]
    unsafe fn dealloc(&mut self, page: PhysicalPage);

    /// # Safety
    ///
    /// The requirements for this method are the same as [`dealloc`], but apply
    /// to the entire range returned
    unsafe fn dealloc_contiguous(&mut self, page: PhysicalPage, n: usize);

    /// # Safety
    ///
    /// The effects of this call should not be memory unsafe, however marking
    /// free pages as used without good reason will eat up the available
    /// physical memory and strain kernel allocations which could result in the
    /// kernel being out of memory, which is very much undesired
    unsafe fn set_used(&mut self, page: PhysicalPage);

    /// # Safety
    ///
    /// You must ensure that page being marked as unused has no remaining
    /// references to it and is wholly unused. Failure to uphold that
    /// requirement could result in undefined behavior if the freed page is then
    /// reallocated to another object in memory, resulting in memory corruption
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
    let ptr = crate::mem::phys2virt(page.as_phys_address()).as_mut_ptr().cast::<u64>();

    unsafe {
        for i in 0..(4096 / 8) {
            *ptr.add(i) = 0;
        }
    }

    page
}
