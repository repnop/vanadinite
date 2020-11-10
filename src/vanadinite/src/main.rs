#![allow(clippy::match_bool)]
#![allow(incomplete_features)]
#![feature(asm, naked_functions, global_asm, alloc_error_handler, raw_ref_op, const_generics)]
#![no_std]
#![no_main]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("vanadinite assumes a 64-bit pointer size, cannot compile on non-64 bit systems");

extern crate alloc;

mod arch;
mod asm;
mod boot;
mod drivers;
mod io;
mod mem;
mod sync;
mod trap;
mod utils;

use arch::csr;
use mem::{
    kernel_patching,
    paging::{PageSize, PhysicalAddress, Read, Sv39PageTable, VirtualAddress, Write, PAGE_TABLE_ROOT},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
};

const TWO_MEBS: usize = 2 * 1024 * 1024;

extern "C" {
    static stvec_trap_shim: utils::LinkerSymbol;
}

#[no_mangle]
unsafe extern "C" fn kmain(_hart_id: usize, fdt: *const u8) -> ! {
    crate::io::init_logging();

    let fdt = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => crate::arch::exit(crate::arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let mut pf_alloc = PHYSICAL_MEMORY_ALLOCATOR.lock();

    let mut page_alloc = || {
        let phys_addr = pf_alloc.alloc().unwrap().as_phys_address();

        (kernel_patching::phys2virt(phys_addr).as_mut_ptr() as *mut Sv39PageTable, phys_addr)
    };

    let root_page_table = &mut *PAGE_TABLE_ROOT.get();

    {
        let uart_addr = 0x10000000;
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

        crate::io::set_console(uart_addr as *mut crate::drivers::misc::uart16550::Uart16550);
    }

    //let uart_fdt = fdt.find_node("/uart");
    //for property in uart_fdt.unwrap() {
    //    if let Some(reg) = property.reg() {
    //        let uart_addr = reg.starting_address() as usize;
    //        let uart_phys = PhysicalAddress::new(uart_addr);
    //        let uart_virt = VirtualAddress::new(uart_addr);
    //        root_page_table.map(
    //            uart_phys,
    //            uart_virt,
    //            PageSize::Kilopage,
    //            Read | Write,
    //            &mut page_alloc,
    //            kernel_patching::phys2virt,
    //        );
    //
    //        crate::io::set_console(uart_addr as *mut crate::drivers::misc::uart16550::Uart16550);
    //    }
    //}

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

    log::info!("SBI spec version: {:?}", sbi::base::spec_version());
    log::info!("SBI implementor: {:?}", sbi::base::impl_id());
    log::info!("marchid: {:#x}", sbi::base::marchid());
    log::info!("Setting stvec to {:#p}", stvec_trap_shim.as_ptr());
    csr::stvec::set(core::mem::transmute(stvec_trap_shim.as_ptr()));

    for child in fdt.find_node("/soc").unwrap().children() {
        println!("{}", child.name);
        for prop in child.properties() {
            println!("    {}: {:?}", prop.name, prop.value);
        }
    }

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
