/*
    SPDX-License-Identifier: MPL-2.0
    SPDX-FileCopyrightText: 2021 The vanadinite developers
    
    This Source Code Form is subject to the terms of the Mozilla Public License,
    v. 2.0. If a copy of the MPL was not distributed with this file, You can
    obtain one at https://mozilla.org/MPL/2.0/.
*/

OUTPUT_ARCH(riscv64imac)

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

    .tdata : AT(ADDR(.tdata) - __offset) {
        . = ALIGN(4K);
        PROVIDE(__tdata_start = .);
        
        *(.tdata .tdata.* .tbss .tbss.*);

        . = ALIGN(4K);
        PROVIDE(__tdata_end = .);
    }

    .tmp_stack : AT(ADDR(.tmp_stack) - __offset) {
        PROVIDE(__tmp_stack_bottom = .);
        . = ALIGN(4K);
        . += 1024 * 1024 * 4;
        . = ALIGN(4K);
        PROVIDE(__tmp_stack_top = .);
    }

    . = ALIGN(2M);
    PROVIDE(KERNEL_END = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
