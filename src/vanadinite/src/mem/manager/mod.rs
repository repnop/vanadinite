// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod address_map;

use crate::{
    mem::{
        paging::{
            flags::{self, Flags},
            PageSize, PageTable, PageTableDebug, PhysicalAddress, VirtualAddress,
        },
        region::{MemoryRegion, PhysicalRegion, UniquePhysicalRegion},
        sfence,
    },
    utils::{self, Units},
};
use address_map::AddressMap;
pub use address_map::{AddressRegion, AddressRegionKind};
use core::ops::Range;

pub enum FillOption<'a> {
    Data(&'a [u8]),
    Unitialized,
    Zeroed,
}

#[derive(Debug)]
pub struct MemoryManager {
    table: PageTable,
    address_map: AddressMap,
}

impl MemoryManager {
    pub fn new() -> Self {
        let mut this = Self { table: PageTable::new(), address_map: AddressMap::new() };

        this.guard(VirtualAddress::new(0));

        this
    }

    /// Allocate a region of memory with an optionally specified address (`None`
    /// will choose a suitable, random address) with the given [`PageSize`], the
    /// number of required pages, with the given permission [`Flags`],
    /// optionally filled or zeroed.
    pub fn alloc_region(
        &mut self,
        at: Option<VirtualAddress>,
        size: PageSize,
        n_pages: usize,
        flags: Flags,
        fill: FillOption<'_>,
        kind: AddressRegionKind,
    ) -> VirtualAddress {
        let at = at.unwrap_or_else(|| self.find_free_region(size, n_pages));

        log::debug!("Allocating region at {:#p}: size={:?} n_pages={} flags={:?}", at, size, n_pages, flags);

        let mut backing = UniquePhysicalRegion::alloc_sparse(size, n_pages);

        match fill {
            FillOption::Data(data) => backing.copy_data_into(data),
            FillOption::Zeroed => backing.zero(),
            FillOption::Unitialized => {}
        }

        let iter = backing.physical_addresses().enumerate().map(|(i, phys)| (phys, at.offset(i * size.to_byte_size())));
        for (phys_addr, virt_addr) in iter {
            log::debug!("Mapping {:#p} -> {:#p}", phys_addr, virt_addr);
            self.table.map(phys_addr, virt_addr, flags, size);
        }

        self.address_map
            .alloc(
                at..at.offset(size.to_byte_size() * n_pages),
                MemoryRegion::Backed(PhysicalRegion::Unique(backing)),
                kind,
            )
            .expect("bad address mapping");

        at
    }

    /// Same as [`Self::alloc_region`], except attempts to find a free region
    /// with available space above and below the region to place guard pages.
    pub fn alloc_guarded_region(
        &mut self,
        size: PageSize,
        n_pages: usize,
        flags: Flags,
        fill: FillOption<'_>,
        kind: AddressRegionKind,
    ) -> VirtualAddress {
        let at = self.find_free_region_with_guards(size, n_pages);

        log::debug!("Allocating guarded region at {:#p}: size={:?} n_pages={} flags={:?}", at, size, n_pages, flags);

        self.guard(VirtualAddress::new(at.as_usize() - 4.kib()));
        self.alloc_region(Some(at), size, n_pages, flags, fill, kind);
        self.guard(at.offset(size.to_byte_size() * n_pages));

        at
    }

    /// Same as [`Self::alloc_region`] except produces a
    /// [`crate::mem::region::SharedPhysicalRegion`] which can be cheaply shared
    /// between tasks
    pub fn alloc_shared_region(
        &mut self,
        at: Option<VirtualAddress>,
        size: PageSize,
        n_pages: usize,
        flags: Flags,
        fill: FillOption<'_>,
        kind: AddressRegionKind,
    ) -> VirtualAddress {
        let at = at.unwrap_or_else(|| self.find_free_region(size, n_pages));
        let mut backing = UniquePhysicalRegion::alloc_sparse(size, n_pages);

        match fill {
            FillOption::Data(data) => backing.copy_data_into(data),
            FillOption::Zeroed => backing.zero(),
            FillOption::Unitialized => {}
        }

        let iter = backing.physical_addresses().enumerate().map(|(i, phys)| (phys, at.offset(i * size.to_byte_size())));
        for (phys_addr, virt_addr) in iter {
            self.table.map(phys_addr, virt_addr, flags, size);
            sfence(Some(virt_addr), None);
        }

        self.address_map
            .alloc(
                at..at.offset(size.to_byte_size() * n_pages),
                MemoryRegion::Backed(PhysicalRegion::Shared(backing.into_shared_region())),
                kind,
            )
            .unwrap();

        at
    }

    /// Place a guard page at the given [`VirtualAddress`]
    pub fn guard(&mut self, at: VirtualAddress) {
        self.address_map.alloc(at..at.offset(4.kib()), MemoryRegion::GuardPage, AddressRegionKind::Guard).unwrap();
        self.table.map(PhysicalAddress::null(), at, flags::USER | flags::VALID, PageSize::Kilopage);
    }

    /// Deallocate the region specified by the given [`VirtualAddress`]
    pub fn dealloc_region(&mut self, at: VirtualAddress) {
        let region = self.address_map.find(at).expect("kernel address passed in");
        assert!(region.region.is_some(), "trying to dealloc an unallocated region");

        let span = region.span.clone();
        let region = self.address_map.free(span).expect("tried deallocing an unmapped region");

        let iter = (0..region.page_count()).map(|i| at.offset(i * region.page_size().to_byte_size()));
        for virt_addr in iter {
            self.table.unmap(virt_addr);
            sfence(Some(virt_addr), None);
        }
    }

    /// Returns the [`AddressRegion`] that contains the given
    /// [`VirtualAddress`], if it exists
    pub fn region_for(&self, at: VirtualAddress) -> Option<&AddressRegion> {
        self.address_map.find(at)
    }

    pub fn map_direct(&mut self, map_from: PhysicalAddress, map_to: VirtualAddress, n_pages: PageSize, flags: Flags) {
        self.table.map(map_from, map_to, flags, n_pages);

        sfence(Some(map_to), None);
    }

    /// Iterates over the given address range, returning `Ok(())` if each page
    /// within the address range satisfied `f`, otherwise returning the first
    /// [`VirtualAddress`] that was not satisfied
    pub fn is_user_region_valid(
        &self,
        range: Range<VirtualAddress>,
        f: impl Fn(Flags) -> bool,
    ) -> Result<(), VirtualAddress> {
        let start = range.start.align_down_to(PageSize::Kilopage);
        let end = range.end.align_to_next(PageSize::Kilopage);

        for page in (start.as_usize()..end.as_usize()).step_by(4.kib()) {
            let page = VirtualAddress::new(page);

            if page.is_kernel_region() {
                return Err(page);
            }

            match self.page_flags(page) {
                Some(flags) if !f(flags) => return Err(page),
                None => return Err(page),
                _ => {}
            }
        }

        Ok(())
    }

    /// Returns the [`Flags`] of the given [`VirtualAddress`], if it's mapped
    pub fn page_flags(&self, virt: VirtualAddress) -> Option<Flags> {
        self.table.page_flags(virt)
    }

    /// Modify the page flags of the given [`VirtualAddress`] mapping, returning
    /// whether or not the mapping exists
    pub fn modify_page_flags(&mut self, virt: VirtualAddress, f: impl FnOnce(Flags) -> Flags) -> bool {
        self.table.modify_page_flags(virt, f)
    }

    /// Returns the `RSW` bits of the given [`VirtualAddress`] mapping, if it's
    /// mapped
    pub fn rsw(&self, virt: VirtualAddress) -> Option<u8> {
        self.table.page_rsw(virt)
    }

    /// Modify the `RSW` bits of the given [`VirtualAddress`] mapping, returning
    /// whether or not the mapping exists
    pub fn modify_rsw(&mut self, virt: VirtualAddress, f: impl FnOnce(Flags) -> Flags) -> bool {
        self.table.modify_page_flags(virt, f)
    }

    /// Attempt to resolve the [`PhysicalAddress`] of the given [`VirtualAddress`] mapping
    pub fn resolve(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        self.table.resolve(virt)
    }

    /// The [`PhysicalAddress`] of the contained [`PageTable`]
    pub fn table_phys_address(&self) -> PhysicalAddress {
        self.table.physical_address()
    }

    /// Debug printable representation of the [`PageTable`]
    pub fn page_table_debug(&self) -> PageTableDebug<'_> {
        self.table.debug()
    }

    /// Debug printable representation of the [`AddressMap`]
    pub fn address_map_debug(&self) -> &AddressMap {
        &self.address_map
    }

    /// Search for an unoccupied memory region that satisfies the given
    /// [`PageSize`] and number of pages. The method will pick a random
    /// [`VirtualAddress`] that is suitable.
    pub fn find_free_region(&self, size: PageSize, n_pages: usize) -> VirtualAddress {
        let total_bytes = n_pages * size.to_byte_size();

        // FIXME: there's probably a better way to do this
        // Try to find a hole big enough 100 times, fall back to linear search otherwise.
        for _ in 0..100 {
            // FIXME: this needs replaced by proper RNG
            let jittered_start = (crate::csr::time::read() * 717) % VirtualAddress::userspace_range().end.as_usize();

            let region = match self.address_map.find(VirtualAddress::new(jittered_start)) {
                Some(r) => r.span.clone(),
                None => continue,
            };

            let aligned_start = VirtualAddress::new(utils::round_up_to_next(jittered_start, size.to_byte_size()));
            let region_size = region.end.as_usize() - aligned_start.as_usize();

            if aligned_start > region.end {
                continue;
            }

            log::debug!("Found unoccupied region: {:#p}-{:#p}", aligned_start, region.end);
            if region_size >= total_bytes {
                return aligned_start;
            }
        }

        for region in self.address_map.unoccupied_regions() {
            let start = region.span.start;
            let end = region.span.end;

            let aligned_start = VirtualAddress::new(utils::round_up_to_next(start.as_usize(), size.to_byte_size()));
            let region_size = end.as_usize() - aligned_start.as_usize();

            if aligned_start > end {
                continue;
            }

            log::debug!("Found unoccupied region: {:#p}-{:#p}", aligned_start, end);
            if region_size >= total_bytes {
                return aligned_start;
            }
        }

        todo!("exhausted address space -- this should be an `Err(...)` in the future")
    }

    fn find_free_region_with_guards(&self, size: PageSize, n_pages: usize) -> VirtualAddress {
        let total_bytes = n_pages * size.to_byte_size();

        // FIXME: there's probably a better way to do this
        // Try to find a hole big enough 100 times, fall back to linear search otherwise.
        for _ in 0..100 {
            // FIXME: this needs replaced by proper RNG
            let jittered_start = (crate::csr::time::read() * 717) % VirtualAddress::userspace_range().end.as_usize();

            let region = match self.address_map.find(VirtualAddress::new(jittered_start)) {
                Some(r) => r.span.clone(),
                None => continue,
            };

            let aligned_start = VirtualAddress::new(utils::round_up_to_next(jittered_start, size.to_byte_size()));

            if aligned_start > region.end {
                continue;
            }

            let above_avail = self.address_map.find(aligned_start.offset(total_bytes)).unwrap().is_unoccupied();
            let below_avail = self.address_map.find(aligned_start.offset(total_bytes)).unwrap().is_unoccupied();

            if !above_avail || !below_avail {
                continue;
            }

            log::debug!("Found unoccupied region: {:#p}-{:#p}", aligned_start, region.end);
            let region_size = region.end.as_usize() - aligned_start.as_usize();
            if region_size >= total_bytes {
                return aligned_start;
            }
        }

        for region in self.address_map.unoccupied_regions() {
            let aligned_start_after_guard = VirtualAddress::new(utils::round_up_to_next(
                region.span.start.offset(4.kib()).as_usize(),
                size.to_byte_size(),
            ));

            if aligned_start_after_guard > region.span.end {
                continue;
            }

            let above_avail =
                self.address_map.find(aligned_start_after_guard.offset(total_bytes)).unwrap().is_unoccupied();

            if !above_avail {
                continue;
            }

            log::debug!("Found unoccupied region: {:#p}-{:#p}", aligned_start_after_guard, region.span.end);
            let region_size = region.span.end.as_usize() - aligned_start_after_guard.as_usize();
            if region_size >= total_bytes {
                return aligned_start_after_guard;
            }
        }

        todo!("exhausted address space -- this should be an `Err(...)` in the future")
    }
}
