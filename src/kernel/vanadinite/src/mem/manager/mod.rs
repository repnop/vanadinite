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

use super::region::SharedPhysicalRegion;

pub enum FillOption<'a> {
    Data(&'a [u8]),
    Unitialized,
    Zeroed,
}

pub enum InvalidRegion {
    NotMapped,
    InvalidPermissions,
}

pub struct RegionDescription<'a> {
    pub size: PageSize,
    pub len: usize,
    pub contiguous: bool,
    pub flags: Flags,
    pub fill: FillOption<'a>,
    pub kind: AddressRegionKind,
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
        description: RegionDescription,
    ) -> Range<VirtualAddress> {
        let RegionDescription { size, len, contiguous, flags, fill, kind } = description;
        let at = at.unwrap_or_else(|| self.find_free_region(size, len));

        log::debug!("Allocating region at {:#p}: size={:?} n_pages={} flags={:?}", at, size, len, flags);

        let mut backing = if contiguous {
            UniquePhysicalRegion::alloc_contiguous(size, len)
        } else {
            UniquePhysicalRegion::alloc_sparse(size, len)
        };

        match fill {
            FillOption::Data(data) => backing.copy_data_into(data),
            FillOption::Zeroed => backing.zero(),
            FillOption::Unitialized => {}
        }

        let iter = backing.physical_addresses().enumerate().map(|(i, phys)| (phys, at.add(i * size.to_byte_size())));
        for (phys_addr, virt_addr) in iter {
            log::trace!("Mapping {:#p} -> {:#p}", phys_addr, virt_addr);
            self.table.map(phys_addr, virt_addr, flags, size);
        }

        let range = at..at.add(size.to_byte_size() * len);
        self.address_map
            .alloc(range.clone(), MemoryRegion::Backed(PhysicalRegion::Unique(backing)), kind)
            .expect("bad address mapping");

        range
    }

    /// Same as [`Self::alloc_region`], except attempts to find a free region
    /// with available space above and below the region to place guard pages.
    pub fn alloc_guarded_region(&mut self, description: RegionDescription) -> VirtualAddress {
        let RegionDescription { size, len, contiguous, flags, fill, kind } = description;
        let at = self.find_free_region_with_guards(size, len);

        log::debug!("Allocating guarded region at {:#p}: size={:?} len={} flags={:?}", at, size, len, flags);

        self.guard(VirtualAddress::new(at.as_usize() - 4.kib()));
        self.alloc_region(Some(at), RegionDescription { size, len, contiguous, flags, fill, kind });
        self.guard(at.add(size.to_byte_size() * len));

        at
    }

    /// Same as [`Self::alloc_region`] except produces a
    /// [`crate::mem::region::SharedPhysicalRegion`] which can be cheaply shared
    /// between tasks
    pub fn alloc_shared_region(
        &mut self,
        at: Option<VirtualAddress>,
        description: RegionDescription,
    ) -> (Range<VirtualAddress>, SharedPhysicalRegion) {
        let RegionDescription { size, len, contiguous, flags, fill, kind } = description;
        let at = at.unwrap_or_else(|| self.find_free_region(size, len));
        let mut backing = if contiguous {
            UniquePhysicalRegion::alloc_contiguous(size, len)
        } else {
            UniquePhysicalRegion::alloc_sparse(size, len)
        };

        match fill {
            FillOption::Data(data) => backing.copy_data_into(data),
            FillOption::Zeroed => backing.zero(),
            FillOption::Unitialized => {}
        }

        let iter = backing.physical_addresses().enumerate().map(|(i, phys)| (phys, at.add(i * size.to_byte_size())));
        for (phys_addr, virt_addr) in iter {
            self.table.map(phys_addr, virt_addr, flags, size);
            sfence(Some(virt_addr), None);
        }

        let shared = backing.into_shared_region();
        let range = at..at.add(size.to_byte_size() * len);

        self.address_map
            .alloc(range.clone(), MemoryRegion::Backed(PhysicalRegion::Shared(shared.clone())), kind)
            .unwrap();

        (range, shared)
    }

    /// # Safety
    /// This function is meant to map MMIO devices into userspace processes, and
    /// will allow aliasing physical memory if used incorrectly.
    ///
    /// Memory regions will be mapped using kilopages (TODO: is larger
    /// granularity necessary?)
    pub unsafe fn map_mmio_device(
        &mut self,
        from: PhysicalAddress,
        to: Option<VirtualAddress>,
        len: usize,
    ) -> (Range<VirtualAddress>, SharedPhysicalRegion) {
        let n_pages = crate::utils::round_up_to_next(4.kib(), len) / 4.kib();
        let at = to.unwrap_or_else(|| self.find_free_region(PageSize::Kilopage, n_pages));

        log::debug!(
            "Mapping MMIO region at {:#p}: phys={:#p} size={:?} n_pages={}",
            at,
            from,
            PageSize::Kilopage,
            n_pages
        );

        let backing = UniquePhysicalRegion::mmio(from, PageSize::Kilopage, n_pages).into_shared_region();

        let iter = backing
            .physical_addresses()
            .enumerate()
            .map(|(i, phys)| (phys, at.add(i * PageSize::Kilopage.to_byte_size())));
        for (phys_addr, virt_addr) in iter {
            log::trace!("Mapping {:#p} -> {:#p}", phys_addr, virt_addr);
            self.table.map(
                phys_addr,
                virt_addr,
                flags::READ | flags::WRITE | flags::USER | flags::VALID,
                PageSize::Kilopage,
            );
        }

        let range = at..at.add(PageSize::Kilopage.to_byte_size() * len);
        self.address_map
            .alloc(
                range.clone(),
                MemoryRegion::Backed(PhysicalRegion::Shared(backing.clone())),
                AddressRegionKind::Mmio,
            )
            .expect("bad address mapping");

        (range, backing)
    }

    pub fn apply_shared_region(
        &mut self,
        at: Option<VirtualAddress>,
        flags: Flags,
        region: SharedPhysicalRegion,
        kind: AddressRegionKind,
    ) -> Range<VirtualAddress> {
        let at = at.unwrap_or_else(|| self.find_free_region(region.page_size(), region.n_pages()));

        let iter = region
            .physical_addresses()
            .enumerate()
            .map(|(i, phys)| (phys, at.add(i * region.page_size().to_byte_size())));

        for (phys_addr, virt_addr) in iter {
            self.table.map(phys_addr, virt_addr, flags, region.page_size());
            sfence(Some(virt_addr), None);
        }

        let range = at..at.add(region.page_size().to_byte_size() * region.n_pages());

        self.address_map.alloc(range.clone(), MemoryRegion::Backed(PhysicalRegion::Shared(region)), kind).unwrap();

        range
    }

    /// Place a guard page at the given [`VirtualAddress`]
    pub fn guard(&mut self, at: VirtualAddress) {
        self.address_map.alloc(at..at.add(4.kib()), MemoryRegion::GuardPage, AddressRegionKind::Guard).unwrap();
        self.table.map(PhysicalAddress::null(), at, flags::USER | flags::VALID, PageSize::Kilopage);
    }

    /// Deallocate the region specified by the given [`VirtualAddress`]
    #[track_caller]
    pub fn dealloc_region(&mut self, at: VirtualAddress) -> MemoryRegion {
        let region = self.address_map.find(at).expect("kernel address passed in");
        assert!(region.region.is_some(), "trying to dealloc an unallocated region");

        let span = region.span.clone();
        let region = self.address_map.free(span).expect("tried deallocing an unmapped region");

        let iter = (0..region.page_count()).map(|i| at.add(i * region.page_size().to_byte_size()));
        for virt_addr in iter {
            self.table.unmap(virt_addr);
            sfence(Some(virt_addr), None);
        }

        region
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
    /// [`VirtualAddress`] that was not satisfied along with the reason for why
    /// it is invalid
    pub fn is_user_region_valid(
        &self,
        range: Range<VirtualAddress>,
        f: impl Fn(Flags) -> bool,
    ) -> Result<(), (VirtualAddress, InvalidRegion)> {
        let start = range.start.align_down_to(PageSize::Kilopage);
        let end = range.end.align_to_next(PageSize::Kilopage);

        for page in (start.as_usize()..end.as_usize()).step_by(4.kib()) {
            let page = VirtualAddress::new(page);

            if page.is_kernel_region() {
                return Err((page, InvalidRegion::InvalidPermissions));
            }

            match self.page_flags(page) {
                Some(flags) if !f(flags) => return Err((page, InvalidRegion::InvalidPermissions)),
                None => return Err((page, InvalidRegion::NotMapped)),
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
            let jittered_start =
                (crate::csr::time::read() as usize * 717) % VirtualAddress::userspace_range().end.as_usize();

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
            let jittered_start =
                (crate::csr::time::read() as usize * 717) % VirtualAddress::userspace_range().end.as_usize();

            let region = match self.address_map.find(VirtualAddress::new(jittered_start)) {
                Some(r) => r.span.clone(),
                None => continue,
            };

            let aligned_start = VirtualAddress::new(utils::round_up_to_next(jittered_start, size.to_byte_size()));

            if aligned_start > region.end {
                continue;
            }

            let above_avail = self.address_map.find(aligned_start.add(total_bytes)).unwrap().is_unoccupied();
            let below_avail = self.address_map.find(aligned_start.add(total_bytes)).unwrap().is_unoccupied();

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
                region.span.start.add(4.kib()).as_usize(),
                size.to_byte_size(),
            ));

            if aligned_start_after_guard > region.span.end {
                continue;
            }

            let above_avail =
                self.address_map.find(aligned_start_after_guard.add(total_bytes)).unwrap().is_unoccupied();

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
