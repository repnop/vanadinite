OUTPUT_ARCH(riscv64gc)

MEMORY {
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 128M
}

ENTRY(_boot);

SECTIONS {
    . = 0x2080000000;
    .text ALIGN(4K) : AT(ADDR(.text) - 0x2080000000) {
        PROVIDE(__kernel_start = .);
        PROVIDE(__start_text = .);
        *(.init.boot .init.rust .init)
        *(.text .text.*)
        PROVIDE(__end_text = .);
    }

    .data ALIGN(4K) : AT(ADDR(.data) - 0x2080000000) {
        *(.data .data.* .rodata .rodata.*)
    }

    . = ALIGN(8);

    .sdata : AT(ADDR(.sdata) - 0x2080000000) {
        PROVIDE(__global_pointer = .);
        *(.sdata .sdata.*)        
    }

    PROVIDE(__bss_start = .);
    .sbss : AT(ADDR(.sbss) - 0x2080000000) {
        *(.sbss .sbss.*);
    }

    .bss : AT(ADDR(.bss) - 0x2080000000) {
        *(.bss .bss.*)
    }

    PROVIDE(__bss_end = .);

    PROVIDE(__kernel_end = .);

    . = ALIGN(4K);

    PROVIDE(__stack_top = . + 0x200000);
    PROVIDE(__scratch_stack = __stack_top + 4K);
    PROVIDE(__heap_start = __scratch_stack);
    PROVIDE(__heap_end = ORIGIN(RAM) + LENGTH(RAM));

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}

ASSERT(__heap_start % 4096 == 0, "heap not on page boundary")