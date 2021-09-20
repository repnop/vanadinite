// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub const fn get_spec_version() -> usize {
    //   [31]    [30..24] [23..0]
    // Reserved   Major    Minor

    const SPEC_MAJOR: usize = 0;
    const SPEC_MINOR: usize = 3;

    (SPEC_MAJOR << 24) | SPEC_MINOR
}

pub const fn get_impl_id() -> usize {
    usize::MAX
}

pub fn get_impl_version() -> usize {
    const fn const_parse(s: &str) -> usize {
        let s = s.as_bytes();
        let mut n = 0;
        let mut i = 0;

        while i < s.len() {
            let b = s[i];
            n *= 10;
            n += (b - b'0') as usize;

            i += 1;
        }

        n
    }

    // FIXME: These should be parsed at compile-time to ensure validity and
    // reduce runtime cost
    const MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
    const MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");

    (const_parse(MAJOR) << 16) | (const_parse(MINOR) & 0xFFFF)
}
