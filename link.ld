OUTPUT_ARCH(riscv64gc)

__kernel_base = 0x2000000000;

/*MEMORY {
    RAM (rwx) : ORIGIN = 0x0080000000, LENGTH = 128M
}
*/

ENTRY(_boot);

SECTIONS {
    . = 0x0080000000;

    . += __kernel_base;
    .text : AT(ADDR(.text) - __kernel_base) {
        PROVIDE(__kernel_start = .);
        PROVIDE(__start_text = .);
        *(.init.boot)
        PROVIDE(___start = .);
        *(.init.rust .init)
        *(.text .text.*)
        PROVIDE(__end_text = .);
    }

    .data ALIGN(4K) : AT(ADDR(.data) - __kernel_base) {
        *(.data .data.* .rodata .rodata.*)
    }

    . = ALIGN(8);

    .sdata : AT(ADDR(.sdata) - __kernel_base) {
        PROVIDE(__global_pointer = .);
        *(.sdata .sdata.*)        
    }

    PROVIDE(__bss_start = .);
    .sbss : AT(ADDR(.sbss) - __kernel_base) {
        *(.sbss .sbss.*);
    }

    .bss : AT(ADDR(.bss) - __kernel_base) {
        *(.bss .bss.*)
    }

    PROVIDE(__bss_end = .);

    PROVIDE(__kernel_end = .);

    . = ALIGN(2M);

    PROVIDE(__stack_top = . + 2M);
    PROVIDE(__scratch_stack = __stack_top + 2M);
    PROVIDE(__heap_start = __scratch_stack);
    PROVIDE(__heap_end = __kernel_start + 128M);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}

ASSERT(__heap_start % 4096 == 0, "heap not on page boundary")