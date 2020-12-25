use paging::PAGE_TABLE_MANAGER;

use self::paging::{PhysicalAddress, VirtualAddress};

// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod heap;
pub mod phys;
pub mod region;
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

#[inline(always)]
pub fn fence() {
    unsafe { asm!("fence") };
}

#[inline(always)]
pub fn satp(mode: SatpMode, asid: u16, root_page_table: paging::PhysicalAddress) {
    let value = ((mode as usize) << 60) | ((asid as usize) << 44) | root_page_table.ppn();
    unsafe { asm!("csrw satp, {}", in(reg) value) };
}

#[repr(usize)]
pub enum SatpMode {
    Bare = 0,
    Sv39 = 8,
    Sv48 = 9,
}

pub fn phys2virt(phys: PhysicalAddress) -> VirtualAddress {
    // let phys_offset = unsafe { *kernel_patching::KERNEL_PHYS_LOAD_LOCATION.0.get() };
    //
    // assert!(phys_offset != 0);

    VirtualAddress::new(phys.as_usize() + 0xFFFFFFC000000000)
}

pub fn virt2phys(virt: VirtualAddress) -> PhysicalAddress {
    //let phys_offset = unsafe { *kernel_patching::KERNEL_PHYS_LOAD_LOCATION.0.get() };
    //
    //assert!(phys_offset != 0);
    //
    //PhysicalAddress::new(virt.as_usize() - kernel_patching::page_offset() + phys_offset)

    PAGE_TABLE_MANAGER.lock().resolve(virt).expect("no mapping found")
}

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

    pub unsafe fn kernel_section_p2v(phys: PhysicalAddress) -> VirtualAddress {
        let phys_offset = *KERNEL_PHYS_LOAD_LOCATION.0.get();
        assert!(phys_offset != 0);
        VirtualAddress::new(phys.as_usize() - phys_offset + page_offset())
    }
}
