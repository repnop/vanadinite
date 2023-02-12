// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod allocator;
pub mod flags;
mod repr;

pub use self::repr::Rsw;
use crate::mem::{
    phys::{PhysicalMemoryAllocator, PhysicalPage},
    phys2virt, virt2phys,
};
use alloc::boxed::Box;
use allocator::PageTableAllocator;
use flags::Flags;
pub use repr::{EntryKind, PageSize, PhysicalAddress, VirtualAddress};

pub struct PageTable {
    root: Box<repr::PageTable, PageTableAllocator>,
}

impl PageTable {
    /// Creates a new [`PageTable`] without copying kernel regions
    pub fn new_raw() -> Self {
        let root = Self::new_table();
        Self { root }
    }

    /// Creates a new [`PageTable`], copying the kernel regions from the active
    /// table
    pub fn new() -> Self {
        // Safety: This is safe since page tables are made up of trivial types,
        // of which zero is a valid state (and the one we want for new ones)
        let root = Self::new_table();
        let mut this = Self { root };

        this.copy_kernel_regions();

        this
    }

    #[track_caller]
    pub fn map(&mut self, from: PhysicalAddress, to: VirtualAddress, flags: Flags, size: PageSize, rsw: Rsw) {
        log::trace!("Mapping {:#p} -> {:#p}", from, to);

        size.assert_addr_aligned(from.as_usize());
        size.assert_addr_aligned(to.as_usize());

        let mut table = &mut *self.root;
        let mut current = PageSize::top_level();

        for vpn in to.vpns().into_iter().rev() {
            let entry = &mut table.entries[vpn];

            if current == size {
                if entry.is_valid() {
                    panic!("attempted to map an already-mapped virtual address: {:#p} -> {:#p}", from, to);
                }

                log::trace!("Map successful: {:#p} to {:#p}", from, to);

                entry.set_flags(flags);
                entry.set_ppn(from);
                entry.set_rsw(rsw);
                return;
            }

            match entry.kind() {
                EntryKind::Leaf => {
                    let entry = *entry;
                    panic!(
                        "already mapped page at a larger page size: from={:#p} to={:#p} flags={:?} size={:?} | current={:?} entry={:?} currently mapped: {:?}",
                        from, to, flags, size, current, entry, self.resolve(to)
                    )
                }
                EntryKind::Branch(paddr) => table = unsafe { &mut *(phys2virt(paddr).as_mut_ptr().cast()) },
                EntryKind::NotValid => {
                    let new_subtable = Box::leak(Self::new_table());
                    let subtable_phys = virt2phys(VirtualAddress::from_ptr(new_subtable));

                    entry.set_flags(Flags::VALID);
                    entry.set_ppn(subtable_phys);

                    table = new_subtable;
                }
            }

            current = match current.next() {
                Some(next) => next,
                None => unreachable!("next level page size"),
            };
        }
    }

    #[track_caller]
    pub fn unmap(&mut self, address: VirtualAddress) {
        log::debug!("Unmapping {:#p}", address);

        let entry = self.with_entry_mut(address, |e, _| (e.is_valid(), *e = repr::PageTableEntry::new()));
        if let None | Some((false, _)) = entry {
            panic!("attempting to unmap and already unmapped page: {:#p}", address);
        }
    }

    pub fn modify_page_flags(&mut self, address: VirtualAddress, f: impl FnOnce(Flags) -> Flags) -> bool {
        self.with_entry_mut(address, |e, _| {
            e.set_flags(f(e.flags()));
            true
        })
        .unwrap_or_default()
    }

    pub fn page_rsw(&self, address: VirtualAddress) -> Option<Rsw> {
        self.with_entry(address, |e, _| e.rsw())
    }

    pub fn modify_page_rsw(&mut self, address: VirtualAddress, f: impl FnOnce(Rsw) -> Rsw) -> bool {
        self.with_entry_mut(address, |e, _| {
            e.set_rsw(f(e.rsw()));
            true
        })
        .unwrap_or_default()
    }

    pub fn page_flags(&self, address: VirtualAddress) -> Option<Flags> {
        self.with_entry(address, |e, _| e.flags())
    }

    pub fn resolve(&self, address: VirtualAddress) -> Option<PhysicalAddress> {
        self.with_entry(address, |e, _| e.ppn()).flatten()
    }

    pub fn physical_address(&self) -> PhysicalAddress {
        virt2phys(VirtualAddress::from_ptr(&*self.root))
    }

    pub fn debug(&self) -> PageTableDebug {
        PageTableDebug(&self.root, PageSize::top_level(), VirtualAddress::new(0))
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn static_map(&mut self, from: PhysicalAddress, to: VirtualAddress, flags: Flags, size: PageSize) {
        size.assert_addr_aligned(from.as_usize());
        size.assert_addr_aligned(to.as_usize());

        let mut table = &mut *self.root;
        let mut current = PageSize::top_level();

        for vpn in to.vpns().into_iter().rev() {
            let entry = &mut table.entries[vpn];
            if current == size {
                if entry.is_valid() {
                    panic!("attempted to map an already-mapped virtual address: {:#p} -> {:#p}", from, to);
                }

                entry.set_flags(flags);
                entry.set_ppn(from);

                return;
            }

            match entry.kind() {
                EntryKind::Leaf => panic!("man ionno, wtf"),
                EntryKind::Branch(paddr) => table = unsafe { &mut *(phys2virt(paddr).as_mut_ptr().cast()) },
                EntryKind::NotValid => {
                    let new_subtable = Box::leak(Self::new_table());
                    let subtable_phys = virt2phys(VirtualAddress::from_ptr(new_subtable));
                    entry.set_flags(Flags::VALID);
                    entry.set_ppn(subtable_phys);

                    table = new_subtable;
                }
            }

            current = match current.next() {
                Some(next) => next,
                None => unreachable!("next level page size"),
            };
        }
    }

    fn with_entry_mut<T>(
        &mut self,
        address: VirtualAddress,
        f: impl FnOnce(&mut repr::PageTableEntry, PageSize) -> T,
    ) -> Option<T> {
        let mut table = &mut *self.root;
        let mut current = PageSize::top_level();

        for vpn in address.vpns().into_iter().rev() {
            let entry = &mut table.entries[vpn];

            match entry.kind() {
                EntryKind::Leaf => return Some(f(entry, current)),
                EntryKind::Branch(paddr) => table = unsafe { &mut *(phys2virt(paddr).as_mut_ptr().cast()) },
                EntryKind::NotValid => return None,
            }

            current = match current.next() {
                Some(next) => next,
                None => unreachable!("next level page size"),
            };
        }

        None
    }

    fn with_entry<T>(
        &self,
        address: VirtualAddress,
        f: impl FnOnce(&repr::PageTableEntry, PageSize) -> T,
    ) -> Option<T> {
        let mut table = &*self.root;
        let mut current = PageSize::top_level();

        for vpn in address.vpns().into_iter().rev() {
            let entry = &table.entries[vpn];

            match entry.kind() {
                EntryKind::Leaf => return Some(f(entry, current)),
                EntryKind::Branch(paddr) => table = unsafe { &*(phys2virt(paddr).as_mut_ptr().cast()) },
                EntryKind::NotValid => return None,
            }

            current = match current.next() {
                Some(next) => next,
                None => unreachable!("next level page size"),
            };
        }

        None
    }

    fn copy_kernel_regions(&mut self) {
        let current: *const repr::PageTable = { phys2virt(crate::csr::satp::read().root_page_table).as_ptr().cast() };

        // FIXME: this address should be available somewhere else and not hardcoded
        let start_idx = *VirtualAddress::kernelspace_range().start.vpns().last().unwrap();
        for i in start_idx..512 {
            self.root.entries[i] = unsafe { (*current).entries[i] };
        }
    }

    fn new_table() -> Box<repr::PageTable, PageTableAllocator> {
        // Safety: the [`PageTableAllocator`] zeroes the page for us, so this is
        // well-defined and safe to immediately init
        unsafe { Box::new_uninit_in(PageTableAllocator).assume_init() }
    }

    fn deallocate(
        phys_mem_allocator: &mut dyn PhysicalMemoryAllocator,
        page_table: *mut repr::PageTable,
        page_size: PageSize,
    ) {
        for entry in unsafe { (*page_table).entries.iter_mut() } {
            match entry.kind() {
                EntryKind::Branch(branch) => {
                    let virt = crate::mem::phys2virt(branch);
                    Self::deallocate(
                        phys_mem_allocator,
                        virt.as_mut_ptr().cast(),
                        page_size.next().expect("Branch found on lowest level page table!"),
                    );

                    unsafe {
                        phys_mem_allocator.dealloc(PhysicalPage::from_ptr(branch.as_mut_ptr()), PageSize::Kilopage)
                    };
                }
                EntryKind::Leaf if entry.rsw() == Rsw::NONE => unsafe {
                    phys_mem_allocator.dealloc(PhysicalPage::from_ptr(entry.ppn().unwrap().as_mut_ptr()), page_size)
                },
                _ => {}
            }
        }
    }
}

unsafe impl Send for PageTable {}
unsafe impl Sync for PageTable {}

impl Drop for PageTable {
    fn drop(&mut self) {
        let mut lock = crate::mem::phys::PHYSICAL_MEMORY_ALLOCATOR.lock();
        Self::deallocate(&mut *lock, &mut *self.root, PageSize::top_level());
    }
}

impl core::fmt::Debug for PageTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PageTable {{ ... }}")
    }
}

pub struct PageTableDebug<'a>(&'a repr::PageTable, PageSize, VirtualAddress);

impl PageTableDebug<'_> {
    fn walk_table(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        for (i, entry) in self.0.entries.iter().enumerate() {
            match entry.kind() {
                EntryKind::NotValid => continue,
                EntryKind::Leaf => {
                    let addr = {
                        let mut vpns = self.2.vpns();
                        vpns[self.1 as usize] = i;
                        VirtualAddress::from_vpns(vpns)
                    };

                    if addr.is_kernel_region() {
                        break;
                    }

                    writeln!(
                        f,
                        "[{}] {:#p} -> {:#p} ({:?})",
                        self.size_to_letter(),
                        addr,
                        entry.ppn().unwrap(),
                        entry.flags(),
                    )?
                }
                EntryKind::Branch(next_level) => {
                    let next_level = unsafe { &*phys2virt(next_level).as_ptr().cast() };
                    let page_size = self.1.next().unwrap();
                    let addr = {
                        let mut vpns = self.2.vpns();
                        vpns[self.1 as usize] = i;
                        VirtualAddress::from_vpns(vpns)
                    };

                    if addr.is_kernel_region() {
                        break;
                    }

                    PageTableDebug(next_level, page_size, addr).walk_table(f)?;
                }
            }
        }

        Ok(())
    }

    fn size_to_letter(&self) -> char {
        match self.1 {
            PageSize::Kilopage => 'K',
            PageSize::Megapage => 'M',
            PageSize::Gigapage => 'G',
            #[cfg(any(feature = "paging.sv48", feature = "paging.sv57"))]
            PageSize::Terapage => 'T',
        }
    }
}

impl core::fmt::Debug for PageTableDebug<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.walk_table(f)
    }
}
