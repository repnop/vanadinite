OUTPUT_ARCH(riscv64gc)

ENTRY(_boot);


SECTIONS {
    . += 0x80000000;
    .text : {
        PROVIDE(__kernel_start = .);
        *(.init.boot)
        *(.init.rust)
        *(.text .text.*)
        PROVIDE(__end_text = .);
    }

    .data : {
        *(.data .data.* .rodata .rodata.*)
        . = ALIGN(8);
        PROVIDE(__tmp_stack = .);
        BYTE(4K);
    }

    . = ALIGN(8);

    .sdata : {
        PROVIDE(__global_pointer$ = .);
        *(.sdata .sdata.*)        
    }

    PROVIDE(__bss_start = .);
    .sbss : {
        *(.sbss .sbss.*);
    }

    .bss : {
        *(.bss .bss.*)
    }

    PROVIDE(__bss_end = .);
    PROVIDE(__kernel_end = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}

/*ASSERT(__heap_start % 4096 == 0, "heap not on page boundary")*/