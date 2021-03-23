// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use {
    core::sync::atomic::{AtomicUsize, Ordering},
    paging::{PhysicalAddress, Sv39PageTable, VirtualAddress},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
};

pub mod heap;
pub mod phys;
pub mod paging {
    mod manager;
    mod perms;
    mod sv39;

    pub use manager::*;
    pub use perms::*;
    pub use sv39::*;
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

pub enum FenceMode {
    Full,
    Read,
    Write,
}

#[inline(always)]
pub fn fence(mode: FenceMode) {
    match mode {
        FenceMode::Full => unsafe { asm!("fence iorw, iorw") },
        FenceMode::Read => unsafe { asm!("fence ir, ir") },
        FenceMode::Write => unsafe { asm!("fence ow, ow") },
    }
}

pub fn alloc_kernel_stack(size: usize) -> *mut u8 {
    assert!(size.is_power_of_two());
    assert_eq!(size % 4096, 0);

    let total_pages = size / 4096;
    let phys_start = unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(total_pages) }.expect("oom :(");

    // FIXME: Eventually make these proper virtual address ranges so we can add
    // guard pages which will detect stack overflowing
    phys2virt(phys_start.as_phys_address().offset(total_pages * 4096)).as_mut_ptr()
}

pub fn phys2virt(phys: PhysicalAddress) -> VirtualAddress {
    VirtualAddress::new(phys.as_usize() + PHYSICAL_OFFSET.load(Ordering::Relaxed))
}

#[track_caller]
pub fn virt2phys(virt: VirtualAddress) -> PhysicalAddress {
    unsafe { &*Sv39PageTable::current() }.translate(virt).expect("no mapping found")
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
