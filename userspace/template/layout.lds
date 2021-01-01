OUTPUT_ARCH(riscv64gc);
ENTRY(_start);

SECTIONS {
    . = 0x20000000;
    .text : {
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
        *(COMMON)
        PROVIDE(__global_pointer$ = .);
        *(.sdata .sdata.*)
    }

    .bss : {
        *(.sbss .sbss.*);
        *(.bss .bss.*)
    }

    . = ALIGN(4K);

    .rodata : {
        *(.rodata .rodata.*)
    }

    /DISCARD/ : { *(.eh_frame .eh_frame.* ); }
}