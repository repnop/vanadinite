// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod free_list;
pub mod slab;

use free_list::FreeListAllocator;

const HEAP_START: usize = 0xFFFFFFD000000000;

#[cfg(any(not(any(feature = "vmalloc.allocator.buddy")), feature = "vmalloc.allocator.freelist"))]
#[global_allocator]
pub static HEAP_ALLOCATOR: FreeListAllocator = FreeListAllocator::new();
