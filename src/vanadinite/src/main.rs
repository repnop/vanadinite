// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

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
unsafe extern "C" fn kmain(hart_id: usize, fdt: *const u8) -> ! {
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

    let stdout = fdt.chosen().and_then(|n| n.stdout());
    if let Some((reg, compatible)) = stdout.and_then(|n| Some((n.reg()?.next()?, n.compatible()?))) {
        let stdout_addr = reg.starting_address as *mut u8;
        let stdout_size = reg.size.unwrap();

        if let Some(device) = crate::io::ConsoleDevices::from_compatible(stdout_addr, compatible) {
            let stdout_phys = PhysicalAddress::from_ptr(stdout_addr);
            let stdout_virt = VirtualAddress::from_ptr(stdout_addr);
            root_page_table.map(
                stdout_phys,
                stdout_virt,
                PageSize::Kilopage,
                Read | Write,
                &mut page_alloc,
                kernel_patching::phys2virt,
            );

            device.set_console();
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

    log::info!(
        "Booted on a {} on hart {}",
        fdt.find_node("/")
            .unwrap()
            .properties()
            .find(|p| p.name == "model")
            .map(|p| core::str::from_utf8(&p.value[..p.value.len() - 1]).unwrap())
            .unwrap(),
        hart_id
    );
    log::info!("SBI spec version: {:?}", sbi::base::spec_version());
    log::info!("SBI implementor: {:?}", sbi::base::impl_id());
    log::info!("marchid: {:#x}", sbi::base::marchid());
    log::info!("Setting stvec to {:#p}", stvec_trap_shim.as_ptr());
    csr::stvec::set(core::mem::transmute(stvec_trap_shim.as_ptr()));

    log::info!("{:?}", fdt.find_node("/uart@10000000").map(|n| n.name));

    for child in fdt.find_node("/").unwrap().children() {
        println!("{}", child.name);
        println!("sizes: {:?}", child.cell_sizes());
        if let Some(compat) = child.compatible() {
            for compatible in compat.all() {
                println!("    compatible with: {:?}", compatible);
            }
        }
    }

    println!("{:?}", fdt.find_node("/soc/interrupt-controller@c000000"));

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
