// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::{paging::PageSize, phys::PhysicalMemoryAllocator},
    utils::round_up_to_next,
    Units,
};

// #[repr(C)]
// struct ControlBlock {
//     dtv: *mut DynamicThreadVector,
// }

/// # Safety
///
/// This function ***must*** be called before any references to any per-hart
/// statics are used, and failing to do so can result in undefined behavior
pub unsafe fn init_thread_locals() {
    use crate::utils::LinkerSymbol;

    extern "C" {
        static __tdata_start: LinkerSymbol;
        static __tdata_end: LinkerSymbol;
    }

    let size = __tdata_end.as_usize() - __tdata_start.as_usize();

    let original_thread_locals = core::slice::from_raw_parts(__tdata_start.as_ptr(), size);
    let new_thread_locals = crate::mem::phys2virt(
        crate::mem::phys::PHYSICAL_MEMORY_ALLOCATOR
            .lock()
            .alloc_contiguous(PageSize::Kilopage, round_up_to_next(size, 4.kib()))
            .unwrap()
            .as_phys_address(),
    )
    .as_mut_ptr();

    core::slice::from_raw_parts_mut(new_thread_locals, size).copy_from_slice(original_thread_locals);

    core::arch::asm!("mv tp, {}", in(reg) new_thread_locals);
}

pub fn tp() -> *mut u8 {
    let val: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) val) };

    val as *mut u8
}
