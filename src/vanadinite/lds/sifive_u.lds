OUTPUT_ARCH(riscv64gc)

ENTRY(_boot);


SECTIONS {
    . = 0xFFFFFFC000000000;
    PROVIDE(PHYSICAL_OFFSET = .);

    . = 0xFFFFFFD000000000;
    PROVIDE(PAGE_OFFSET = .);

    __offset = . - 0x80200000;
    .text : AT(ADDR(.text) - __offset) {
        PROVIDE(__text_start = .);
        PROVIDE(KERNEL_START = .);
        *(.init.boot)
        *(.init.rust)
        *(.text .text.*)
        . = ALIGN(4K);
        PROVIDE(__text_end = .);
    }

    .data : AT(ADDR(.data) - __offset) {
        PROVIDE(__data_start = .);
        *(.data .data.* .rodata .rodata.*)
        . = ALIGN(8);
        PROVIDE(__tmp_stack_bottom = .);
        . += 1024 * 1024 * 8;
        PROVIDE(__tmp_stack_top = .);
        . += 4096;
        PROVIDE(__scratch_stack = .);
        . = ALIGN(8);
    }

    . = ALIGN(8);

    .sdata : AT(ADDR(.sdata) - __offset) {
        PROVIDE(__global_pointer$ = .);
        *(.sdata .sdata.*)       
        . = ALIGN(4K);
        PROVIDE(__data_end = .); 
    }

    PROVIDE(__bss_start = .);
    .sbss : AT(ADDR(.sbss) - __offset) {
        *(.sbss .sbss.*);
    }

    .bss : AT(ADDR(.bss) - __offset) {
        *(.bss .bss.*)
    }
    . = ALIGN(4K);
    PROVIDE(__bss_end = .);

    .kernel_thread_local : AT(ADDR(.kernel_thread_local) - __offset) {
        . = ALIGN(4K);
        PROVIDE(__kernel_thread_local_start = .);
        *(.kernel_thread_local .kernel_thread_local.*)
        . = ALIGN(4K);
        PROVIDE(__kernel_thread_local_end = .);
    }

    . = ALIGN(2M);
    PROVIDE(KERNEL_END = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
