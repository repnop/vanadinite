use crate::mem::{paging::VirtualAddress, satp, virt2phys, SatpMode};

extern "C" {
    fn return_to_usermode(_: &crate::trap::Registers, sepc: extern "C" fn()) -> !;
}

pub fn context_switch(process: &crate::process::Process) -> ! {
    satp(SatpMode::Sv39, 1, virt2phys(VirtualAddress::from_ptr(process.page_table.table())));

    log::info!("scheduling! {:#p}", process.pc);

    for _ in 0..1000000000 {
        unsafe { asm!("nop") };
    }

    unsafe { return_to_usermode(&process.frame.registers, process.pc) }
}

#[rustfmt::skip]
global_asm!("
    .section .text
    .globl return_to_usermode
    .align 4
    return_to_usermode:
        csrw sepc, a1

        li t0, 1 << 8
        csrc sstatus, t0
        li t0, 1 << 19
        csrs sstatus, t0
        
        ld x1, 0(a0)
        ld x2, 8(a0)
        ld x3, 16(a0)
        ld x4, 24(a0)
        ld x5, 32(a0)
        ld x6, 40(a0)
        ld x7, 48(a0)
        ld x8, 56(a0)
        ld x9, 64(a0)
        ld x11, 80(a0)
        ld x12, 88(a0)
        ld x13, 96(a0)
        ld x14, 104(a0)
        ld x15, 112(a0)
        ld x16, 120(a0)
        ld x17, 128(a0)
        ld x18, 136(a0)
        ld x19, 144(a0)
        ld x20, 152(a0)
        ld x21, 160(a0)
        ld x22, 168(a0)
        ld x23, 176(a0)
        ld x24, 184(a0)
        ld x25, 192(a0)
        ld x26, 200(a0)
        ld x27, 208(a0)
        ld x28, 216(a0)
        ld x29, 224(a0)
        ld x30, 232(a0)
        ld x31, 240(a0)

        ld x10, 72(a0)
        sret
");
