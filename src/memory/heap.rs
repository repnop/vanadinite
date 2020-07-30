use alloc::alloc::GlobalAlloc;

mod heap_private {
    use crate::util::LinkerSymbol;

    extern "C" {
        pub static mut __heap_start: LinkerSymbol;
        pub static mut __heap_end: LinkerSymbol;
    }
}

pub fn heap_start() -> *mut u8 {
    unsafe { heap_private::__heap_start.as_mut_ptr() }
}

pub fn heap_end() -> *mut u8 {
    unsafe { heap_private::__heap_end.as_mut_ptr() }
}

const PAGE_SIZE_IN_BYTES: usize = 4096;

pub struct MassiveWasteOfHeap;

pub static DO_TRACE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);

#[global_allocator]
static ALLOCATOR: MassiveWasteOfHeap = MassiveWasteOfHeap;

unsafe impl GlobalAlloc for MassiveWasteOfHeap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut current_page = heap_start();

        while (current_page as usize) < (heap_end() as usize) {
            let sentinel = current_page.add(PAGE_SIZE_IN_BYTES - 1).read();

            match sentinel {
                0x69 => current_page = current_page.add(PAGE_SIZE_IN_BYTES),
                _ => {
                    current_page.add(PAGE_SIZE_IN_BYTES - 1).write(0x69);

                    if DO_TRACE.load(core::sync::atomic::Ordering::SeqCst) {
                        log::info!(
                            "Allocation request succeeded: {:?} @ {:#p}",
                            layout,
                            current_page
                        );
                    }
                    return current_page;
                }
            }
        }

        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        if DO_TRACE.load(core::sync::atomic::Ordering::SeqCst) {
            log::info!("Deallocation request: {:?} @ {:#p}", layout, ptr);
        }

        ptr.add(PAGE_SIZE_IN_BYTES - 1).write(0x00);
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("alloc error! layout: {:?}", layout)
}
