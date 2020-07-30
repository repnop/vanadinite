OUTPUT_ARCH(riscv64gc)

MEMORY {
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 128M
}

ENTRY(_start);

SECTIONS {
    .text : {
        PROVIDE(__start_text = .);
        *(.init.rust .init)
        *(.text .text.*)
        PROVIDE(__end_text = .);
    } >RAM

    .rodata : {
        *(.rodata .rodata.*)
    } >RAM

    .data : {
        *(.data .data.*)
    } >RAM

    . = ALIGN(8);

    .sdata : {
        PROVIDE(__global_pointer = .);
        *(.sdata .sdata.*)        
    } >RAM

    PROVIDE(__bss_start = .);
    .sbss : {
        *(.sbss .sbss.*);
    } >RAM

    .bss : {
        *(.bss .bss.*)
    } >RAM

    PROVIDE(__bss_end = .);

    . = ALIGN(4K);

    PROVIDE(__stack_top = . + 0x200000);
    PROVIDE(__scratch_stack = __stack_top + 4K);
    PROVIDE(__heap_start = __scratch_stack);
    PROVIDE(__heap_end = ORIGIN(RAM) + LENGTH(RAM));

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}

ASSERT(__heap_start % 4096 == 0, "heap not on page boundary")