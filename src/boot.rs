use crate::kernel_entry;

#[link_section = ".init.rust"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() -> ! {
    let hart_id;
    let fdt;

    #[rustfmt::skip]
    asm!("
        .option push
        .option norelax
        la gp, __global_pointer$
        .option pop
        la sp, __stack_top
        add s0, sp, zero
        mv {}, a0
        mv {}, a1
    ", out(reg) hart_id , out(reg) fdt);

    clear_bss();

    kernel_entry(hart_id, fdt);

    #[allow(unreachable_code)]
    asm!("1: wfi", "j 1b");

    loop {}
}

#[inline(always)]
pub unsafe fn clear_bss() {
    extern "C" {
        static __bss_start: *mut u8;
        static __BSS_END__: *mut u8;
    }

    let as_slice =
        core::slice::from_raw_parts_mut(__bss_start, __BSS_END__ as usize - __bss_start as usize);

    for byte in as_slice.iter_mut() {
        *byte = 0;
    }
}
