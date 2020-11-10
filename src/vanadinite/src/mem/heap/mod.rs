pub mod free_list;

use free_list::FreeListAllocator;

const HEAP_START: usize = 0xFFFFFFD000000000;

#[cfg(any(not(any(feature = "vmalloc.allocator.buddy")), feature = "vmalloc.allocator.freelist"))]
#[global_allocator]
pub static PHYSICAL_MEMORY_ALLOCATOR: FreeListAllocator = FreeListAllocator::new();
