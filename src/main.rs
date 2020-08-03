#![feature(asm, naked_functions, global_asm, alloc_error_handler)]
#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod virt;

mod asm;
mod boot;
mod fdt;
mod locked;
mod mem;
mod trap;
mod utils;

use alloc::{boxed::Box, string::String};
use core::convert::TryInto;
use log::info;

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn kernel_entry(hart_id: usize, fdt: *const u8) -> ! {
    unsafe {
        asm!("nop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\n");
    }
    virt::init_uart_logger();

    info!("log test!");
    info!("{:#p}", trap::trap_handler as *const u8);
    info!(
        "mhartid: {} (we got {} from QEMU), mvendorid: {}",
        asm::mhartid(),
        hart_id,
        asm::mvendorid()
    );

    let misa = asm::misa();
    let extensions = misa.extensions();

    info!("Extensions available: {}", extensions);

    info!(
        "Heap start: {:p}, end: {:p}",
        mem::heap::heap_start(),
        mem::heap::heap_end(),
    );

    let heap_size = mem::heap::heap_end() as usize - mem::heap::heap_start() as usize;

    info!("We have {} MiB of heap available", heap_size / 1024 / 1024);

    use mem::{
        paging::{Permissions, Sv39PageTable, Sv39PageTableEntry},
        PhysicalAddress, VirtualAddress,
    };
    let mut pt1 = Sv39PageTable::new();
    let mut pt2 = Sv39PageTable::new();
    let mut pt3 = Sv39PageTable::new();

    pt1[0x03].validate_or_else(|| {
        let mut pg = Sv39PageTableEntry::new();
        pg.set_next_page_table(&pt2);

        pg
    });

    pt2[0xF5].validate_or_else(|| {
        let mut pg = Sv39PageTableEntry::new();
        pg.set_next_page_table(&pt3);

        pg
    });

    pt3[0xDB].validate_or_else(|| {
        let mut pg = Sv39PageTableEntry::new();
        pg.set_ppn(0xCAFEB000 as *const u8);
        pg.set_permissions(Permissions::ReadWrite);

        pg
    });

    info!(
        "{:x?}",
        VirtualAddress(0xDEADBEEF).to_physical_address(&pt1)
    );

    let fdt = unsafe { fdt::Fdt::from_ptr(fdt) };
    let node = fdt.find("memory").unwrap();
    let mem_info = &node["reg"];
    let size = u64::from_be_bytes(mem_info.value()[8..].try_into().unwrap());
    let at = u64::from_be_bytes(mem_info.value()[..8].try_into().unwrap());
    info!(
        "we have {} MiB RAM starting @ {:#x}",
        size / 1024 / 1024,
        at
    );

    //const MROM: *const u8 = 0x1020 as *const u8;
    //
    //for i in (0..(0x11000 - 0x100)).step_by(16) {
    //    let mut chars = [' '; 16];
    //
    //    let mrom_iter: &[u8; 16] = unsafe { &*(MROM.add(i).cast()) };
    //    for (i, byte) in mrom_iter.iter().copied().enumerate() {
    //        print!("{:0>2x} ", byte);
    //        if byte >= 0x20 && byte <= 0x7F {
    //            chars[i] = byte as char;
    //        }
    //    }
    //
    //    print!("  |  ");
    //
    //    for c in chars.iter().copied() {
    //        print!("{}", c);
    //    }
    //
    //    println!();
    //    let mut locked = virt::uart::UART0.lock();
    //    let _ = locked.read();
    //}

    let boxed_value = Box::new(5);
    info!("{:?}", boxed_value);
    drop(boxed_value);

    let mut repl = utils::repl::Repl::new();

    repl.run();

    virt::exit(virt::ExitStatus::Pass);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    virt::exit(virt::ExitStatus::Fail(1));

    // #[allow(clippy::empty_loop)]
    // loop {}
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!("we've aborted")
}
