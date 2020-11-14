// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub const EXTENSION_ID: usize = 0x54494D45;

pub fn set_timer(time: u64) {
    unsafe {
        asm!(
            "ecall",
            in("a0") time,
            inout("a6") 0 => _,
            inout("a7") EXTENSION_ID => _,
        );
    }
}
