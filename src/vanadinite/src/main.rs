#![allow(clippy::match_bool)]
#![feature(asm, naked_functions, global_asm, alloc_error_handler, raw_ref_op)]
#![no_std]
#![no_main]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("vanadinite assumes a 64-bit pointer size, cannot compile on non-64 bit systems");

extern crate alloc;

struct Heck;

unsafe impl alloc::alloc::GlobalAlloc for Heck {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        todo!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        todo!()
    }
}

#[global_allocator]
static HECK: Heck = Heck;

mod arch;
mod asm;
mod boot;
mod drivers;
mod io;
mod mem;
mod sync;
// mod trap;
mod utils;

use mem::{
    kernel_patching,
    paging::{PageSize, PhysicalAddress, Read, Sv39PageTable, VirtualAddress, Write, PAGE_TABLE_ROOT},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
};

const TWO_MEBS: usize = 2 * 1024 * 1024;

#[no_mangle]
unsafe extern "C" fn kmain(hart_id: usize, fdt: *const u8) -> ! {
    if hart_id != 0 {
        panic!("not hart 0");
    }

    crate::io::init_logging();

    let fdt = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => crate::arch::exit(crate::arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let mut pf_alloc = PHYSICAL_MEMORY_ALLOCATOR.lock();

    let phys = pf_alloc.alloc().unwrap().as_phys_address();

    let mut page_alloc = || {
        let phys_addr = pf_alloc.alloc().unwrap().as_phys_address();
        //let virt_addr = kernel_patching::phys2virt(phys_addr);

        (kernel_patching::phys2virt(phys_addr).as_mut_ptr() as *mut Sv39PageTable, phys_addr)
    };

    let root_page_table = &mut *PAGE_TABLE_ROOT.get();
    root_page_table.map(
        phys,
        VirtualAddress::new(0xFFFFFFE000000000),
        PageSize::Kilopage,
        Read | Write,
        &mut page_alloc,
        kernel_patching::phys2virt,
    );

    let uart_fdt = fdt.find_node("/uart");
    for property in uart_fdt.unwrap() {
        if let Some(reg) = property.reg() {
            let uart_addr = reg.starting_address() as usize;
            let uart_phys = PhysicalAddress::new(uart_addr);
            let uart_virt = VirtualAddress::new(uart_addr);
            root_page_table.map(
                uart_phys,
                uart_virt,
                PageSize::Kilopage,
                Read | Write,
                &mut page_alloc,
                kernel_patching::phys2virt,
            );

            crate::io::set_console(uart_addr as *mut crate::drivers::uart16550::Uart16550);
        }
    }

    // Remove identity mapping after paging initialization
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;
    for address in (kernel_start..kernel_end).step_by(TWO_MEBS) {
        // `kernel_start()` and `kernel_end()` now refer to virtual addresses so
        // we need to patch them back to physical "virtual" addresses to be
        // unmapped
        let patched = VirtualAddress::new(kernel_patching::virt2phys(VirtualAddress::new(address)).as_usize());
        root_page_table.unmap(patched, kernel_patching::phys2virt);
    }

    let mut pf_alloc = PHYSICAL_MEMORY_ALLOCATOR.lock();

    let page = pf_alloc.alloc().unwrap();
    log::info!("{:?}", page);
    pf_alloc.dealloc(page);

    drop(pf_alloc);

    log::info!("# of memory reservations: {}\r", fdt.memory_reservations().len());
    log::info!("{:#x?}", fdt.memory());

    log::info!("SBI spec version: {:?}", sbi::base::spec_version());
    log::info!("SBI implementor: {:?}", sbi::base::impl_id());
    log::info!("marchid: {:#x}", sbi::base::marchid());

    arch::exit(arch::ExitStatus::Ok)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    arch::exit(arch::ExitStatus::Error(info))
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!("we've aborted")
}

#[alloc_error_handler]
fn alloc_error_handler(_: alloc::alloc::Layout) -> ! {
    panic!()
}
