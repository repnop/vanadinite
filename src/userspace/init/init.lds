/*
    SPDX-License-Identifier: MPL-2.0
    SPDX-FileCopyrightText: 2021 The vanadinite developers
    
    This Source Code Form is subject to the terms of the Mozilla Public License,
    v. 2.0. If a copy of the MPL was not distributed with this file, You can
    obtain one at https://mozilla.org/MPL/2.0/.
*/

OUTPUT_ARCH(riscv64gc)

ENTRY(_start);

SECTIONS {
    . = 0xF00D0000;

    .text : {
        *(.rt.entry)
        *(.text .text.*)
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    . = ALIGN(8);

    .sdata : {
        PROVIDE(__global_pointer$ = . + 0x800);
        *(.sdata .sdata.*)
    }

    PROVIDE(__bss_start = .);
    .sbss : {
        *(.sbss .sbss.*);
    }

    .bss : {
        *(.bss .bss.*)
        PROVIDE(__bss_end = .);
        PROVIDE(end = .);
    }
    . = ALIGN(4K);

    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}
