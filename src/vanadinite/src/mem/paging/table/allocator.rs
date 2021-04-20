// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use {
    crate::mem::{
        phys::{zalloc_page, PhysicalPage},
        phys2virt, virt2phys, PhysicalMemoryAllocator, VirtualAddress, PHYSICAL_MEMORY_ALLOCATOR,
    },
    core::{
        alloc::{AllocError, Layout},
        ptr::NonNull,
    },
};

pub struct PageTableAllocator;

unsafe impl alloc::alloc::Allocator for PageTableAllocator {
    #[track_caller]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert_eq!(layout.align(), 4096, "attempted to allocate something other than a page table");

        let page = phys2virt(zalloc_page().as_phys_address()).as_mut_ptr();

        Ok(unsafe { NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(page, 4096)) })
    }

    #[track_caller]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        assert_eq!(layout.align(), 4096, "attempted to allocate something other than a page table");

        PHYSICAL_MEMORY_ALLOCATOR
            .lock()
            .dealloc(PhysicalPage::from_ptr(virt2phys(VirtualAddress::from_ptr(ptr.as_ptr())).as_mut_ptr()));
    }
}
