#![no_std]
#![no_main]

#[macro_use]
mod virt;

mod asm;
mod locked;
mod memory;
mod trap;

use log::{debug, info};

#[no_mangle]
pub extern "C" fn kernel_entry() -> ! {
    virt::init_uart_logger();

    info!("log test!");
    debug!(
        "mhartid: {}, mvendorid: {}",
        asm::mhartid(),
        asm::mvendorid()
    );

    let misa = asm::misa();
    let extensions = misa.extensions();

    info!("Extensions available: {}", extensions);

    use memory::{
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

    debug!(
        "{:#x?}",
        VirtualAddress(0xDEADBEEF).to_physical_address(&pt1)
    );

    loop {
        let mut locked = virt::uart::UART0.lock();
        let c = locked.read();
        drop(locked);
        println!("{}", c as char);
        if c == 4 {
            break;
        }
    }

    virt::exit(virt::ExitStatus::Pass);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    virt::exit(virt::ExitStatus::Fail(1));

    // #[allow(clippy::empty_loop)]
    // loop {}
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!("we've aborted")
}
