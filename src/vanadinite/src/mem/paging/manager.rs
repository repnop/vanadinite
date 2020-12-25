// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    interrupts::InterruptDisabler,
    mem::phys2virt,
    mem::{
        paging::{PageSize, PhysicalAddress, Read, Sv39PageTable, ToPermissions, VirtualAddress, Write},
        phys::PhysicalMemoryAllocator,
        sfence,
    },
    sync::Mutex,
    utils::StaticMut,
    PHYSICAL_MEMORY_ALLOCATOR,
};

pub static PAGE_TABLE_MANAGER: Mutex<PageTableManager> = Mutex::new(PageTableManager);

static PAGE_TABLE_ROOT: StaticMut<Sv39PageTable> = StaticMut::new(Sv39PageTable::new());

pub struct PageTableManager;

impl PageTableManager {
    pub fn alloc_virtual_range<P: ToPermissions + Copy>(&mut self, start: VirtualAddress, size: usize, perms: P) {
        assert_eq!(size % 4096, 0, "bad map range size: {}", size);

        for idx in 0..size / 4096 {
            self.alloc_virtual(start.offset(idx * 4096), perms);
        }
    }

    pub fn alloc_virtual<P: ToPermissions>(&mut self, map_to: VirtualAddress, perms: P) {
        let _disabler = InterruptDisabler::new();
        let phys = Self::new_phys_page();

        log::info!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        unsafe { &mut *PAGE_TABLE_ROOT.get() }.map(
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

    pub fn map_direct<P: ToPermissions>(
        &mut self,
        map_from: PhysicalAddress,
        map_to: VirtualAddress,
        size: PageSize,
        perms: P,
    ) {
        let _disabler = InterruptDisabler::new();
        //log::info!("PageTableManager::map_page: mapping {:#p} to {:#p}", map_from, map_to);
        unsafe { &mut *PAGE_TABLE_ROOT.get() }.map(
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

    pub fn resolve(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        let _disabler = InterruptDisabler::new();

        unsafe { &*PAGE_TABLE_ROOT.get() }.translate(virt, phys2virt)
    }

    pub unsafe fn set_satp(&mut self) {
        crate::mem::satp(crate::mem::SatpMode::Sv39, 0, PhysicalAddress::from_ptr(PAGE_TABLE_ROOT.get()));
    }

    pub unsafe fn map_with_allocator<F, A, P>(
        &mut self,
        map_from: PhysicalAddress,
        map_to: VirtualAddress,
        page_size: PageSize,
        perms: P,
        f: F,
        translation: A,
    ) where
        F: FnMut() -> (*mut Sv39PageTable, PhysicalAddress),
        A: Fn(PhysicalAddress) -> VirtualAddress,
        P: ToPermissions,
    {
        let _disabler = InterruptDisabler::new();

        { &mut *PAGE_TABLE_ROOT.get() }.map(map_from, map_to, page_size, perms, f, translation);
    }

    /// Memory from this function is never freed since it could be invalid to free it with normal means
    pub unsafe fn unmap_with_translation<A>(&mut self, map_to: VirtualAddress, translation: A)
    where
        A: Fn(PhysicalAddress) -> VirtualAddress,
    {
        let _disabler = InterruptDisabler::new();

        { &mut *PAGE_TABLE_ROOT.get() }.unmap(map_to, translation);
    }

    pub unsafe fn is_mapped_with_translation<A>(&mut self, addr: VirtualAddress, translation: A) -> bool
    where
        A: Fn(PhysicalAddress) -> VirtualAddress,
    {
        let _disabler = InterruptDisabler::new();
        { &mut *PAGE_TABLE_ROOT.get() }.is_mapped(addr, translation)
    }

    fn new_phys_page() -> PhysicalAddress {
        unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("we oom, rip") }.as_phys_address()
    }
}
