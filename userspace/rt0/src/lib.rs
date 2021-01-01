#![feature(asm, naked_functions, start, lang_items)]
#![no_std]

#[start]
#[naked]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    #[rustfmt::skip]
    asm!("
        .align 4
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        mv a0, zero
        mv a1, zero
        call lang_start

        mv a1, a0
        li a0, 0
        ecall
    ", options(noreturn));
}

#[no_mangle]
extern "C" fn lang_start(_: isize, _: *const *const u8) -> isize {
    unsafe { main() };
    0
}

extern "Rust" {
    static main: fn();
}
