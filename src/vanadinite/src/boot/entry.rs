// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

/// # Safety
/// I'm the kernel, rustc
#[naked]
#[no_mangle]
#[link_section = ".init.boot"]
pub unsafe extern "C" fn _boot() -> ! {
    #[rustfmt::skip]
    asm!("
        csrw sie, zero
        csrci sstatus, 2
        
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        lla t0, __bss_start
        lla t1, __bss_end

        clear_bss:
            beq t0, t1, done_clear_bss
            sd zero, (t0)
            addi t0, t0, 8
            j clear_bss

        done_clear_bss:

        lla sp, __tmp_stack_top

        lla a2, PAGE_OFFSET
        lla t0, KERNEL_PHYS_LOAD_LOCATION
        sd a2, (t0)

        j early_paging

        .section .data
        .globl kmain_addr_virt
        kmain_addr_virt: .dword kmain
        .globl PAGE_OFFSET_VALUE
        PAGE_OFFSET_VALUE: .dword PAGE_OFFSET
    ", options(noreturn));
}
