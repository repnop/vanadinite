.text
    .global _start
    _start:
        la sp, __stack
        la t0, trap_handler_inner
        csrw mtvec, t0
        jal clear_bss
        jal kernel_entry

    hcf:
        wfi
        j hcf

    clear_bss:
        la t0, __bss_start__
        la t1, __bss_end__
        zero_bss:
            sw zero, 0(t0)
            addi t0, t0, 4
            blt t0, t1, zero_bss # if t0 < t1 then loop
        ret

    .align 8
    trap_handler_inner:
        addi sp, sp, -8
        sd t0, 0(sp)
        la t0, registers
        sd x0, 0(t0)
        sd x1, 8(t0)
        addi sp, sp, 8
        sd x2, 16(t0)
        addi sp, sp, -8
        sd x3, 24(t0)
        sd x4, 32(t0)
        sd x5, 40(t0)
        sd x6, 48(t0)
        sd x7, 56(t0)
        sd x8, 64(t0)
        sd x9, 72(t0)
        sd x10, 80(t0)
        sd x11, 88(t0)
        sd x12, 96(t0)
        sd x13, 104(t0)
        sd x14, 112(t0)
        sd x15, 120(t0)
        sd x16, 128(t0)
        sd x17, 136(t0)
        sd x18, 144(t0)
        sd x19, 152(t0)
        sd x20, 160(t0)
        sd x21, 168(t0)
        sd x22, 176(t0)
        sd x23, 184(t0)
        sd x24, 192(t0)
        sd x25, 200(t0)
        sd x26, 208(t0)
        sd x27, 216(t0)
        sd x28, 224(t0)
        sd x29, 232(t0)
        sd x30, 240(t0)
        sd x31, 248(t0)
        la t1, registers
        ld t0, 0(sp)
        sd t0, 40(t1)

        mv a0, t1
        jal trap_handler
        addi sp, sp, 8
        ret

.data
.align 8
registers: .dword 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
