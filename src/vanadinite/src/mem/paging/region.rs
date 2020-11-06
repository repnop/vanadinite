use crate::{
    mem::{
        paging::{PageSize, PhysicalAddress, Read, Sv39PageTable, ToPermissions, VirtualAddress, Write},
        phys::PhysicalMemoryAllocator,
    },
    sync::Mutex,
    utils::LinkerSymbol,
    PAGE_TABLE_ROOT, PHYSICAL_MEMORY_ALLOCATOR,
};

pub trait MemoryRegion {}

const PAGE_TABLE_OFFSET: usize = 0xFFFFFFE000000000;

pub fn phys2virt(phys: PhysicalAddress) -> VirtualAddress {
    VirtualAddress::new(phys.as_usize() + PAGE_TABLE_OFFSET)
}

pub fn virt2phys(virt: VirtualAddress) -> PhysicalAddress {
    PhysicalAddress::new(virt.as_usize() - PAGE_TABLE_OFFSET)
}

pub struct PageTableManager {}

impl PageTableManager {
    pub fn map_page<P: ToPermissions>(&mut self, perms: P, map_to: VirtualAddress) {
        let phys = Self::new_page();

        log::info!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        unsafe { &mut *PAGE_TABLE_ROOT.get() }.map(
            phys,
            map_to,
            PageSize::Kilopage,
            perms,
            Self::map_page_table_page,
            phys2virt,
        );
    }

    fn map_page_table_page() -> (*mut Sv39PageTable, PhysicalAddress) {
        let phys = Self::new_page();

        //log::info!("PageTableManager::map_page_table_page: mapping new page table to {:#p}", phys);
        //
        //root.map(phys, phys2virt(phys), PageSize::Kilopage, Read | Write, Self::map_page_table_page, phys2virt);
        //
        //log::info!("PageTableManager::map_page_table_page: zeroing new page table");
        let page = phys2virt(phys).as_mut_ptr();
        for i in 0..4096 {
            unsafe { *page.add(i) = 0 };
        }

        (page.cast(), phys)
    }

    fn new_page() -> PhysicalAddress {
        unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("we oom, rip") }.as_phys_address()
    }
}
