#![no_std]
#![no_main]

#[macro_use]
mod virt;

mod asm;
mod locked;
mod paging;
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

    virt::exit(virt::Finisher::Pass, 0);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    #[allow(clippy::empty_loop)]
    loop {}
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!("we've aborted")
}
