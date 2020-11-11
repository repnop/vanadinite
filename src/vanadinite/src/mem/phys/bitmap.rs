use super::{PhysicalMemoryAllocator, PhysicalPage};

const SINGLE_ENTRY_SIZE_BYTES: usize = 64 * 4096;
const FULL_ENTRY: u64 = 0xFFFF_FFFF_FFFF_FFFF;

pub struct BitmapAllocator {
    bitmap: [u64; 4096],
    mem_start: *mut u8,
    mem_end: *mut u8,
}

impl BitmapAllocator {
    pub const fn new() -> Self {
        Self { bitmap: [0; 4096], mem_start: core::ptr::null_mut(), mem_end: core::ptr::null_mut() }
    }
}

unsafe impl PhysicalMemoryAllocator for BitmapAllocator {
    #[track_caller]
    fn init(&mut self, start: *mut u8, end: *mut u8) {
        assert_eq!(start as usize % 4096, 0, "unaligned memory start page");
        self.mem_start = start;
        self.mem_end = end;
    }

    #[track_caller]
    unsafe fn alloc(&mut self) -> Option<PhysicalPage> {
        let mut page = None;

        if let Some((index, entry)) = self.bitmap.iter_mut().enumerate().find(|(_, e)| **e != FULL_ENTRY) {
            let bit_index = entry.trailing_ones() as usize;

            let page_ptr = (self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES) + (bit_index * 4096);
            let page_ptr = page_ptr as *mut u8;

            if page_ptr <= self.mem_end {
                page = Some(PhysicalPage(page_ptr));
                *entry |= 1 << bit_index;
            }
        }

        page
    }

    #[track_caller]
    unsafe fn dealloc(&mut self, page: PhysicalPage) {
        let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
        let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;

        let entry = &mut self.bitmap[index];

        if (*entry >> bit) & 1 != 1 {
            panic!(
                "[pmalloc.allocator] BitmapAllocator::dealloc: double free detected for address {:#p}",
                page.as_phys_address().as_ptr()
            );
        }

        *entry &= !(1 << bit);
    }

    #[track_caller]
    unsafe fn set_used(&mut self, page: PhysicalPage) {
        let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
        let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;

        let entry = &mut self.bitmap[index];

        *entry |= 1 << bit;
    }

    #[track_caller]
    unsafe fn set_unused(&mut self, page: PhysicalPage) {
        let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
        let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;

        let entry = &mut self.bitmap[index];

        *entry &= !(1 << bit);
    }
}

unsafe impl Send for BitmapAllocator {}
unsafe impl Sync for BitmapAllocator {}
