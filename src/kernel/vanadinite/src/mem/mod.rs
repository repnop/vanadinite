// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use {
    core::{
        arch::asm,
        sync::atomic::{AtomicUsize, Ordering},
    },
    paging::{PageSize, PhysicalAddress, VirtualAddress},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
};

// pub static KERNEL_MEMORY_MANAGER: SpinMutex<manager::UserspaceMemoryManager> =
//     SpinMutex::new(manager::UserspaceMemoryManager::new());

pub mod heap;
pub mod manager;
pub mod phys;
pub mod region;
pub mod user;
pub mod paging {
    mod table;
    #[cfg(test)]
    mod tests;

    use crate::csr::satp::SatpMode;
    pub use table::*;

    #[cfg(all(not(feature = "paging.sv48"), not(feature = "paging.sv57")))]
    pub const SATP_MODE: SatpMode = SatpMode::Sv39;

    #[cfg(all(feature = "paging.sv48", not(feature = "paging.sv57")))]
    pub const SATP_MODE: SatpMode = SatpMode::Sv48;
}

#[inline(always)]
pub fn sfence(vaddr: Option<paging::VirtualAddress>, asid: Option<u16>) {
    unsafe {
        match (vaddr, asid) {
            (Some(vaddr), Some(asid)) => {
                let vaddr = vaddr.as_usize();
                asm!("sfence.vma {}, {}", in(reg) vaddr, in(reg) asid);
            }
            (Some(vaddr), None) => {
                let vaddr = vaddr.as_usize();
                asm!("sfence.vma {}, zero", in(reg) vaddr);
            }
            (None, Some(asid)) => asm!("sfence.vma zero, {}", in(reg) asid),
            (None, None) => asm!("sfence.vma zero, zero"),
        }
    }
}

pub fn alloc_kernel_stack(size: usize) -> *mut u8 {
    assert!(size.is_power_of_two());
    assert_eq!(size % 4096, 0);

    let total_pages = size / 4096;
    let phys_start =
        unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(PageSize::Kilopage, total_pages) }.expect("oom :(");

    // FIXME: Eventually make these proper virtual address ranges so we can add
    // guard pages which will detect stack overflowing
    phys2virt(phys_start.as_phys_address().offset(total_pages * 4096)).as_mut_ptr()
}

#[track_caller]
pub fn phys2virt(phys: PhysicalAddress) -> VirtualAddress {
    VirtualAddress::new(phys.offset(PHYSICAL_OFFSET.load(Ordering::Relaxed)).as_usize())
}

#[track_caller]
pub fn virt2phys(virt: VirtualAddress) -> PhysicalAddress {
    PhysicalAddress::new(virt.as_usize() - PHYSICAL_OFFSET.load(Ordering::Relaxed))
}

pub static PHYSICAL_OFFSET: AtomicUsize = AtomicUsize::new(0);

pub mod kernel_patching {
    use crate::utils;
    use core::cell::UnsafeCell;

    use super::paging::{PhysicalAddress, VirtualAddress};

    extern "C" {
        static KERNEL_START: utils::LinkerSymbol;
        static KERNEL_END: utils::LinkerSymbol;
        static PAGE_OFFSET_VALUE: usize;
    }

    #[repr(transparent)]
    pub(super) struct StaticUsize(pub UnsafeCell<usize>);

    unsafe impl Send for StaticUsize {}
    unsafe impl Sync for StaticUsize {}

    #[no_mangle]
    pub(super) static KERNEL_PHYS_LOAD_LOCATION: StaticUsize = StaticUsize(UnsafeCell::new(0));

    #[inline(always)]
    pub fn page_offset() -> usize {
        unsafe { PAGE_OFFSET_VALUE }
    }

    pub fn kernel_start() -> *const u8 {
        unsafe { KERNEL_START.as_ptr() }
    }

    pub fn kernel_end() -> *const u8 {
        unsafe { KERNEL_END.as_ptr() }
    }

    /// # Safety
    ///
    /// The physical address passed in must be inside the kernel sections,
    /// otherwise the resulting [`VirtualAddress`] will be invalid
    pub unsafe fn kernel_section_p2v(phys: PhysicalAddress) -> VirtualAddress {
        let phys_offset = *KERNEL_PHYS_LOAD_LOCATION.0.get();
        assert!(phys_offset != 0);
        VirtualAddress::new(phys.as_usize() - phys_offset + page_offset())
    }

    /// # Safety
    ///
    /// The virtual address passed in must be inside the kernel sections,
    /// otherwise the resulting [`PhysicalAddress`] will be invalid
    pub unsafe fn kernel_section_v2p(virt: VirtualAddress) -> PhysicalAddress {
        let phys_offset = *KERNEL_PHYS_LOAD_LOCATION.0.get();
        assert!(phys_offset != 0);
        PhysicalAddress::new(virt.as_usize() - page_offset() + phys_offset)
    }
}
