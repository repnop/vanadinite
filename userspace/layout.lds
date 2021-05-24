/*
    SPDX-License-Identifier: MPL-2.0
    SPDX-FileCopyrightText: 2021 The vanadinite developers
    
    This Source Code Form is subject to the terms of the Mozilla Public License,
    v. 2.0. If a copy of the MPL was not distributed with this file, You can
    obtain one at https://mozilla.org/MPL/2.0/.
*/

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

    . = ALIGN(4K);

    .tdata : {
        *(.tdata .tdata.*)
    }

    .tbss : {
        *(.tbss .tbss.*)
    }

    . = ALIGN(4K);

    .capabilities : {
        KEEP(*(.capabilities))
    }

    . = ALIGN(4K);
    
    /DISCARD/ : { *(.eh_frame .eh_frame.* ); }
}