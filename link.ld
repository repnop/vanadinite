OUTPUT_ARCH(riscv64gc)

MEMORY {
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 128M
}

ENTRY(_boot);

SECTIONS {
    . = 0x80000000;
    PROVIDE(__kernel_start = 0x80000000);
    PROVIDE(__kernel_size = __kernel_end - __kernel_start);
    
    . = 0xFFFFF80000000000;
    .text : AT(ADDR(.text) - 0xFFFFF80000000000 + 0x80000000) {
        PROVIDE(__start_text = .);
        *(.init.rust .init)
        *(.text .text.*)
        PROVIDE(__end_text = .);
    } >RAM

    .rodata : AT(ADDR(.rodata) - 0xFFFFF80000000000 + 0x80000000) {
        *(.rodata .rodata.*)
    } >RAM

    .data : AT(ADDR(.data) - 0xFFFFF80000000000 + 0x80000000) {
        *(.data .data.*)
    } >RAM

    . = ALIGN(8);

    .sdata : AT(ADDR(.sdata) - 0xFFFFF80000000000 + 0x80000000) {
        PROVIDE(__global_pointer = .);
        *(.sdata .sdata.*)        
    } >RAM

    PROVIDE(__bss_start = .);
    .sbss : AT(ADDR(.sbss) - 0xFFFFF80000000000 + 0x80000000) {
        *(.sbss .sbss.*);
    } >RAM

    .bss : AT(ADDR(.bss) - 0xFFFFF80000000000 + 0x80000000) {
        *(.bss .bss.*)
    } >RAM

    PROVIDE(__bss_end = .);

    PROVIDE(__kernel_end = . - 0xFFFFF80000000000);

    . = ALIGN(4K);

    PROVIDE(__stack_top = . + 0x200000);
    PROVIDE(__scratch_stack = __stack_top + 4K);
    PROVIDE(__heap_start = __scratch_stack);
    PROVIDE(__heap_end = ORIGIN(RAM) + LENGTH(RAM));

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}

ASSERT(__heap_start % 4096 == 0, "heap not on page boundary")