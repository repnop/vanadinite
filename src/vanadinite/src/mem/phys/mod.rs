pub mod bitmap;

use crate::{mem::paging::PhysicalAddress, sync::Mutex};
use bitmap::BitmapAllocator;

#[cfg(any(not(any(feature = "pmalloc.allocator.buddy")), feature = "pmalloc.allocator.bitmap"))]
pub static PHYSICAL_MEMORY_ALLOCATOR: Mutex<BitmapAllocator> = Mutex::new(BitmapAllocator::new());

pub unsafe trait PhysicalMemoryAllocator {
    fn init(&mut self, start: *mut u8, end: *mut u8);
    unsafe fn alloc(&mut self) -> Option<PhysicalPage>;
    unsafe fn dealloc(&mut self, page: PhysicalPage);
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
