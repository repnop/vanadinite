use crate::{
    kernel_patching::phys2virt,
    mem::{
        paging::{PageSize, PhysicalAddress, Read, Sv39PageTable, ToPermissions, VirtualAddress, Write},
        phys::PhysicalMemoryAllocator,
    },
    sync::Mutex,
    PAGE_TABLE_ROOT, PHYSICAL_MEMORY_ALLOCATOR,
};

pub static PAGE_TABLE_MANAGER: Mutex<PageTableManager> = Mutex::new(PageTableManager);

pub struct PageTableManager;

impl PageTableManager {
    pub fn map_range<P: ToPermissions + Copy>(&mut self, perms: P, start: VirtualAddress, end: VirtualAddress) {
        assert!(start.as_usize() < end.as_usize());

        for addr in (start.as_usize()..=end.as_usize()).step_by(4096) {
            self.map_page(perms, VirtualAddress::new(addr));
        }
    }

    pub fn map_page<P: ToPermissions>(&mut self, perms: P, map_to: VirtualAddress) {
        let phys = Self::new_page();

        log::info!("PageTableManager::map_page: mapping {:#p} to {:#p}", phys, map_to);
        unsafe { &mut *PAGE_TABLE_ROOT.get() }.map(
            phys,
            map_to,
            PageSize::Kilopage,
            perms,
            || {
                let phys = Self::new_page();
                let virt = phys2virt(phys).as_mut_ptr().cast();

                unsafe {
                    *virt = Sv39PageTable::default();
                }

                (virt, phys)
            },
            phys2virt,
        );
    }

    fn new_page() -> PhysicalAddress {
        unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("we oom, rip") }.as_phys_address()
    }
}
