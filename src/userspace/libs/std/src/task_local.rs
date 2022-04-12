// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ffi::c_void;

const DTV_OFFSET: isize = 0x800;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct TlsIndex {
    module: usize,
    offset: isize,
}

#[no_mangle]
unsafe extern "C" fn __tls_get_addr(index: *const TlsIndex) -> *mut c_void {
    assert_eq!((*index).module, 1, "whelp guess we need to handle multiple modules");
    (*dtv()).gen_then_modules[(*index).module].offset((*index).offset + DTV_OFFSET)
}

unsafe fn dtv() -> *mut DynamicThreadVector {
    (*tcb()).dtv
}

unsafe fn tcb() -> *mut ThreadControlBlock {
    let tcb: *mut ThreadControlBlock;
    core::arch::asm!("addi {}, tp, -24", out(reg) tcb);
    tcb
}

#[repr(C)]
struct ThreadControlBlock {
    dtv: *mut DynamicThreadVector,
}

#[repr(C)]
struct DynamicThreadVector {
    gen_then_modules: [*mut c_void; 2],
}
