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

pub struct PageRange<A: Address> {
    pub start: A,
    pub end: A,
    pub page_size: PageSize,
}

impl<A: Address> PageRange<A> {
    #[track_caller]
    pub fn new(start: A, end: A, page_size: PageSize) -> Self {
        page_size.assert_addr_aligned(start.address());
        page_size.assert_addr_aligned(end.address());

        Self { start, end, page_size }
    }

    pub fn into_std_range(self) -> core::ops::Range<A> {
        self.start..self.end
    }
}

impl<A: Address> IntoIterator for PageRange<A> {
    type IntoIter = PageRangeIter<A>;
    type Item = A;

    fn into_iter(self) -> Self::IntoIter {
        PageRangeIter { current: self.start, end: self.end, page_size: self.page_size }
    }
}

impl<A: Address> IntoIterator for &'_ PageRange<A> {
    type IntoIter = PageRangeIter<A>;
    type Item = A;

    fn into_iter(self) -> Self::IntoIter {
        PageRangeIter { current: self.start, end: self.end, page_size: self.page_size }
    }
}

impl<A: Address> Copy for PageRange<A> {}
impl<A: Address> Clone for PageRange<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Address + core::fmt::Debug> core::fmt::Debug for PageRange<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageRange")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("page_size", &self.page_size)
            .finish()
    }
}

pub struct PageRangeIter<A: Address> {
    current: A,
    end: A,
    page_size: PageSize,
}

impl<A: Address> Iterator for PageRangeIter<A> {
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.address() == self.end.address() {
            return None;
        }

        let new = self.current.checked_add(self.page_size.to_byte_size())?;
        Some(core::mem::replace(&mut self.current, new))
    }
}

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for VirtualAddress {}
impl sealed::Sealed for PhysicalAddress {}
pub trait Address: sealed::Sealed + Sized + Copy {
    fn address(self) -> usize;
    fn from_address(addr: usize) -> Self;
    fn checked_add(self, amount: usize) -> Option<Self>;
}

impl Address for VirtualAddress {
    fn address(self) -> usize {
        self.as_usize()
    }

    fn from_address(addr: usize) -> Self {
        Self::new(addr)
    }

    fn checked_add(self, amount: usize) -> Option<Self> {
        self.checked_add(amount)
    }
}

impl Address for PhysicalAddress {
    fn address(self) -> usize {
        self.as_usize()
    }

    fn from_address(addr: usize) -> Self {
        Self::new(addr)
    }

    fn checked_add(self, amount: usize) -> Option<Self> {
        Some(Self::new(self.as_usize() + amount))
    }
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
