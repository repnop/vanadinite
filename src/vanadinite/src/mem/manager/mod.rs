// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod address_map;

use crate::{
    mem::{
        paging::{flags::Flags, PageSize, PageTable, PageTableDebug, PhysicalAddress, VirtualAddress},
        region::{PhysicalRegion, UniquePhysicalRegion},
        sfence,
    },
    utils::Units,
};
use address_map::AddressMap;
use core::ops::Range;

#[derive(Debug)]
pub struct MemoryManager {
    table: PageTable,
    address_map: AddressMap,
}

impl MemoryManager {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { table: PageTable::new(), address_map: AddressMap::new(VirtualAddress::userspace_range()) }
    }

    pub fn alloc_region(&mut self, at: VirtualAddress, n_pages: usize, flags: Flags, fill_with: Option<&[u8]>) {
        let mut backing = UniquePhysicalRegion::alloc_sparse(n_pages);
        if let Some(fill_with) = fill_with {
            backing.copy_data_into(fill_with);
        }

        let iter = backing.physical_addresses().enumerate().map(|(i, phys)| (phys, at.offset(i * 4.kib())));
        for (phys_addr, virt_addr) in iter {
            self.table.map(phys_addr, virt_addr, flags, PageSize::Kilopage);
            sfence(Some(virt_addr), None);
        }

        self.address_map
            .alloc(at..at.offset(4.kib() * n_pages), PhysicalRegion::Unique(backing))
            .expect("bad address mapping");
    }

    pub fn alloc_shared_region(&mut self, at: VirtualAddress, n_pages: usize, flags: Flags, fill_with: Option<&[u8]>) {
        let mut backing = UniquePhysicalRegion::alloc_sparse(n_pages);
        if let Some(fill_with) = fill_with {
            backing.copy_data_into(fill_with);
        }

        let iter = backing.physical_addresses().enumerate().map(|(i, phys)| (phys, at.offset(i * 4.kib())));
        for (phys_addr, virt_addr) in iter {
            self.table.map(phys_addr, virt_addr, flags, PageSize::Kilopage);
            sfence(Some(virt_addr), None);
        }

        self.address_map
            .alloc(at..at.offset(4.kib() * n_pages), PhysicalRegion::Shared(backing.into_shared_region()))
            .unwrap();
    }

    pub fn dealloc_region(&mut self, at: VirtualAddress) {
        let region = self.address_map.find(at).expect("kernel address passed in");
        assert!(region.region.is_some(), "trying to dealloc an unallocated region");

        let span = region.span.clone();
        let region = self.address_map.free(span).expect("tried deallocing an unmapped region");

        let iter = (0..region.page_count()).map(|i| at.offset(i * 4.kib()));
        for virt_addr in iter {
            self.table.unmap(virt_addr);
            sfence(Some(virt_addr), None);
        }
    }

    pub fn map_direct(&mut self, map_from: PhysicalAddress, map_to: VirtualAddress, n_pages: PageSize, flags: Flags) {
        self.table.map(map_from, map_to, flags, n_pages);

        sfence(Some(map_to), None);
    }

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

    pub fn page_flags(&self, virt: VirtualAddress) -> Option<Flags> {
        self.table.page_flags(virt)
    }

    pub fn modify_page_flags(&mut self, virt: VirtualAddress, f: impl FnOnce(Flags) -> Flags) -> bool {
        self.table.modify_page_flags(virt, f)
    }

    pub fn rsw(&self, virt: VirtualAddress) -> Option<Flags> {
        self.table.page_flags(virt)
    }

    pub fn modify_rsw(&mut self, virt: VirtualAddress, f: impl FnOnce(Flags) -> Flags) -> bool {
        self.table.modify_page_flags(virt, f)
    }

    pub fn resolve(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        self.table.resolve(virt)
    }

    pub fn table_phys_address(&self) -> PhysicalAddress {
        self.table.physical_address()
    }

    pub fn page_table_debug(&self) -> PageTableDebug<'_> {
        self.table.debug()
    }

    //pub fn debug_print(&self) -> PageTableDebugPrint {
    //    PageTableDebugPrint(self.0)
    //}
}

unsafe impl Send for PageTable {}
unsafe impl Sync for PageTable {}

//pub struct PageTableDebugPrint(*mut Sv39PageTable);
//
//impl core::fmt::Debug for PageTableDebugPrint {
//    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//        let end_n = VirtualAddress::new(0xFFFFFFC000000000).vpns()[2];
//        writeln!(f, "\n")?;
//        for gib_entry_i in 0..end_n {
//            let gib_entry = &self.table.entries[gib_entry_i];
//            let next_table = match gib_entry.kind() {
//                EntryKind::Leaf => {
//                    writeln!(
//                        f,
//                        "[G] {:#p} => {:#p}",
//                        VirtualAddress::new(gib_entry_i << 30),
//                        gib_entry.ppn().unwrap()
//                    )?;
//                    continue;
//                }
//                EntryKind::NotValid => continue,
//                EntryKind::Branch(phys) => unsafe { &*phys2virt(phys).as_mut_ptr().cast::<Sv39PageTable>() },
//            };
//
//            for mib_entry_i in 0..512 {
//                let mib_entry = &next_table.entries[mib_entry_i];
//                let next_table = match mib_entry.kind() {
//                    EntryKind::Leaf => {
//                        writeln!(
//                            f,
//                            "[M] {:#p} => {:#p}",
//                            VirtualAddress::new((gib_entry_i << 30) | (mib_entry_i << 21)),
//                            mib_entry.ppn().unwrap()
//                        )?;
//                        continue;
//                    }
//                    EntryKind::NotValid => continue,
//                    EntryKind::Branch(phys) => unsafe { &*phys2virt(phys).as_mut_ptr().cast::<Sv39PageTable>() },
//                };
//
//                for kib_entry_i in 0..512 {
//                    let kib_entry = &next_table.entries[kib_entry_i];
//                    match kib_entry.kind() {
//                        EntryKind::Leaf => {
//                            writeln!(
//                                f,
//                                "[K] {:#p} => {:#p}",
//                                VirtualAddress::new((gib_entry_i << 30) | (mib_entry_i << 21) | (kib_entry_i << 12)),
//                                kib_entry.ppn().unwrap()
//                            )?;
//                            continue;
//                        }
//                        EntryKind::NotValid => continue,
//                        EntryKind::Branch(_) => unreachable!("A KiB PTE was marked as a branch?"),
//                    }
//                }
//            }
//        }
//        writeln!(f, "\n")?;
//
//        Ok(())
//    }
//}
