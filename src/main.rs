#![allow(clippy::match_bool)]
#![feature(asm, naked_functions, global_asm, alloc_error_handler)]
#![no_std]
#![no_main]

mod arch;
mod asm;
mod fdt;
mod sync;
mod trap;
mod utils;

/// # Safety
/// I'm the kernel, rustc
#[naked]
#[no_mangle]
#[link_section = ".init.boot"]
pub unsafe extern "C" fn _boot() -> ! {
    #[rustfmt::skip]
    asm!("
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        lla sp, __tmp_stack

        j boot_entry
    ");

    loop {}
}
/// # Safety
/// Uh, probably none
#[naked]
#[no_mangle]
#[link_section = ".init.rust"]
pub unsafe extern "C" fn boot_entry(hart_id: usize, fdt: *const u8) -> ! {
    const LOOK_AT_ME_IM_THE_CAPTAIN_NOW: u8 = 0b0001_1111;

    asm!("csrwi pmpcfg0, {pmpcfg}", pmpcfg = const LOOK_AT_ME_IM_THE_CAPTAIN_NOW);
    asm!("li t0, -1", out("t0") _);
    asm!("csrw pmpaddr0, t0");

    if hart_id != 0 {
        panic!("not hart 0");
    }

    let header = match fdt::FdtHeader::new(fdt) {
        Some(header) => header,
        None => arch::exit(arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

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
