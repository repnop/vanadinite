// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

enum Void {}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct TlsIndex {
    module: usize,
    offset: isize,
}

#[no_mangle]
unsafe extern "C" fn __tls_get_addr(_index: *const TlsIndex) -> *mut Void {
    let tp: *mut Void;
    core::arch::asm!("mv {}, tp", out(reg) tp, options(nostack));

    tp
}
