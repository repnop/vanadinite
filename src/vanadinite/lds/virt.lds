OUTPUT_ARCH(riscv64gc)

ENTRY(_boot);


SECTIONS {
    /* . = 0x80000000; */
    . = 0xffffffff80000000;
    PROVIDE(PAGE_OFFSET = .);
    .text : AT(ADDR(.text) - 0xFFFFFFFEFFE00000) {
        PROVIDE(KERNEL_START = .);
        *(.init.boot)
        *(.init.rust)
        *(.text .text.*)
        PROVIDE(__end_text = .);
    }

    . = ALIGN(4K);

    .data : AT(ADDR(.data) - 0xFFFFFFFEFFE00000) {
        *(.data .data.* .rodata .rodata.*)
        . = ALIGN(8);
        PROVIDE(__tmp_stack_bottom = .);
        . += 1024 * 1024 * 8;
        PROVIDE(__tmp_stack_top = .);
        . = ALIGN(8);
    }

    . = ALIGN(8);

    .sdata : AT(ADDR(.sdata) - 0xFFFFFFFEFFE00000) {
        PROVIDE(__global_pointer$ = .);
        *(.sdata .sdata.*)        
    }

    . = ALIGN(4K);

    PROVIDE(__bss_start = .);
    .sbss : AT(ADDR(.sbss) - 0xFFFFFFFEFFE00000) {
        *(.sbss .sbss.*);
    }

    .bss : AT(ADDR(.bss) - 0xFFFFFFFEFFE00000) {
        *(.bss .bss.*)
    }
    . = ALIGN(8);
    PROVIDE(__bss_end = .);

    . = ALIGN(2M);
    PROVIDE(KERNEL_END = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}

PROVIDE(HEAP_START = 0xffffff9000000000);