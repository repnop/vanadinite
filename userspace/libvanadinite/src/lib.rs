#![feature(asm)]
#![no_std]

extern crate rt0;

#[inline(always)]
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

#[inline(always)]
pub fn print<T: AsRef<[u8]> + ?Sized>(value: &T) {
    let value = value.as_ref();
    unsafe {
        #[rustfmt::skip]
        asm!(
            "li a0, 1",
            "mv a1, {}",
            "mv a2, {}",
            "ecall",
            in(reg) value.as_ptr(),
            in(reg) value.len(),
            out("a0") _,
            out("a1") _,
            out("a2") _,
        );
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    exit()
}
