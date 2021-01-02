#![feature(asm, naked_functions, start, lang_items)]
#![no_std]

#[start]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    #[rustfmt::skip]
    asm!("
        .align 4
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop
    ");

    main();

    #[rustfmt::skip]
    asm!("
        mv a1, a0
        li a0, 0
        ecall
    ", options(noreturn));
}

extern "Rust" {
    fn main();
}
