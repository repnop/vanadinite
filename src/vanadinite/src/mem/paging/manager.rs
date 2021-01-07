// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    interrupts::InterruptDisabler,
    mem::phys2virt,
    mem::{
        paging::{PageSize, PhysicalAddress, Sv39PageTable, ToPermissions, VirtualAddress},
        phys::PhysicalMemoryAllocator,
        sfence,
    },
    PHYSICAL_MEMORY_ALLOCATOR,
};

#[derive(Debug)]
pub struct PageTableManager(&'static mut Sv39PageTable);

impl PageTableManager {
    pub fn new(table: &'static mut Sv39PageTable) -> Self {
        Self(table)
    }

    pub fn alloc_virtual_range<P: ToPermissions + Copy>(&mut self, start: VirtualAddress, size: usize, perms: P) {
        assert_eq!(size % 4096, 0, "bad map range size: {}", size);

        for idx in 0..size / 4096 {
            self.alloc_virtual(start.offset(idx * 4096), perms);
        }
    }

    pub fn alloc_virtual<P: ToPermissions>(&mut self, map_to: VirtualAddress, perms: P) {
        let _disabler = InterruptDisabler::new();
        let phys = Self::new_phys_page();

        log::debug!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        self.0.map(
            phys,
            map_to,
            PageSize::Kilopage,
            perms,
            || {
                let phys = Self::new_phys_page();
                let virt = phys2virt(phys).as_mut_ptr().cast();

                unsafe {
                    *virt = Sv39PageTable::default();
                }

                (virt, phys)
            },
            phys2virt,
        );

        sfence(Some(map_to), None);
    }

    pub fn alloc_virtual_range_with_data<P: ToPermissions + Copy>(
        &mut self,
        start: VirtualAddress,
        size: usize,
        perms: P,
        data: &[u8],
    ) {
        assert_eq!(size % 4096, 0, "bad map range size: {}", size);

        for (idx, data) in (0..size / 4096).zip(data.chunks(4096)) {
            self.alloc_virtual_with_data(start.offset(idx * 4096), perms, data);
        }
    }

    pub fn alloc_virtual_with_data<P: ToPermissions>(&mut self, map_to: VirtualAddress, perms: P, data: &[u8]) {
        let _disabler = InterruptDisabler::new();
        let phys = Self::new_phys_page();

        log::debug!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        self.0.map(
            phys,
            map_to,
            PageSize::Kilopage,
            perms,
            || {
                let phys = Self::new_phys_page();
                let virt = phys2virt(phys).as_mut_ptr().cast();

                unsafe {
                    *virt = Sv39PageTable::default();
                }

                (virt, phys)
            },
            phys2virt,
        );

        let ptr = phys2virt(phys).as_mut_ptr();

        for (i, byte) in data.iter().copied().enumerate() {
            unsafe { *ptr.add(i) = byte };
        }

        sfence(Some(map_to), None);
    }

    pub fn map_direct<P: ToPermissions>(
        &mut self,
        map_from: PhysicalAddress,
        map_to: VirtualAddress,
        size: PageSize,
        perms: P,
    ) {
        let _disabler = InterruptDisabler::new();
        self.0.map(
            map_from,
            map_to,
            size,
            perms,
            || {
                let phys = Self::new_phys_page();
                let virt = phys2virt(phys).as_mut_ptr().cast();

                unsafe {
                    *virt = Sv39PageTable::default();
                }

                (virt, phys)
            },
            phys2virt,
        );

        sfence(Some(map_to), None);
    }

    pub fn modify_page_permissions(&mut self, virt: VirtualAddress, new_permissions: impl ToPermissions) {
        if let Some(entry) = self.0.entry_mut(virt, phys2virt) {
            entry.set_permissions(new_permissions);
        }
    }

    pub fn resolve(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        let _disabler = InterruptDisabler::new();

        self.0.translate(virt, phys2virt)
    }

    pub fn table(&self) -> &Sv39PageTable {
        &self.0
    }

    pub fn copy_kernel_pages(&mut self) {
        let current = unsafe { &*Sv39PageTable::current() };

        let start_idx = VirtualAddress::new(0xFFFFFFC000000000).vpns()[2];
        for i in start_idx..512 {
            self.0.entries[i] = current.entries[i];
        }
    }

    pub fn is_valid_readable(&self, virt: VirtualAddress) -> bool {
        match self.0.entry(virt, phys2virt) {
            Some(entry) => entry.is_readable(),
            None => false,
        }
    }

    fn new_phys_page() -> PhysicalAddress {
        unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("we oom, rip") }.as_phys_address()
    }
}
