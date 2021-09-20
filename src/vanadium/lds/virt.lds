/*
    SPDX-License-Identifier: MPL-2.0
    SPDX-FileCopyrightText: 2021 The vanadinite developers
    
    This Source Code Form is subject to the terms of the Mozilla Public License,
    v. 2.0. If a copy of the MPL was not distributed with this file, You can
    obtain one at https://mozilla.org/MPL/2.0/.
*/

OUTPUT_ARCH(riscv64gc)

ENTRY(_entry);


SECTIONS {
    . = 0x80000000;
    .text : {
        PROVIDE(__text_start = .);
        PROVIDE(VANADIUM_START = .);
        *(.boot.entry)
        *(.text .text.*)
        PROVIDE(__text_end = .);
    }

    .data : {
        *(.data .data.* .rodata .rodata.*)
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

    .tdata : {
        *(.tdata .tdata.*)
    }

    .tmp_stack : {
        PROVIDE(__tmp_stack_bottom = .);
        . = ALIGN(8);
        . += 1024 * 8;
        PROVIDE(__tmp_stack_top = .);
    }

    . = ALIGN(4K);
    PROVIDE(VANADIUM_END = .);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
