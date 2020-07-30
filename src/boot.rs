use crate::{kernel_entry, util::LinkerSymbol};

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
        la t0, mtvec_trap_shim
        csrw mtvec, t0
        csrr t0, mstatus
        li t1, 1
        slli t1, t1, 13
        or t0, t0, t1
        csrw mstatus, t0
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
        static mut __bss_start: LinkerSymbol;
        static mut __bss_end: LinkerSymbol;
    }

    let as_slice = core::slice::from_raw_parts_mut(
        __bss_start.as_mut_ptr(),
        __bss_end.as_mut_ptr() as usize - __bss_start.as_mut_ptr() as usize,
    );

    for byte in as_slice.iter_mut() {
        *byte = 0;
    }
}
