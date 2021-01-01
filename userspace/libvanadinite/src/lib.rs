#![feature(asm)]
#![no_std]

extern crate rt0;

pub fn exit() -> ! {
    unsafe {
        #[rustfmt::skip]
        asm!(
            "mv a0, zero",
            "ecall",
            options(noreturn),
        );
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    exit()
}
