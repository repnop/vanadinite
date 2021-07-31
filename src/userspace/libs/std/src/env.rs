// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[no_mangle]
static mut ARGS: [usize; 2] = [0; 2];

pub fn args() -> &'static [&'static str] {
    let [argc, argv] = unsafe { ARGS };

    match [argc, argv] {
        [0, _] | [_, 0] => &[],
        [argc, argv] => unsafe { core::slice::from_raw_parts(argv as *const &str, argc) },
    }
}
