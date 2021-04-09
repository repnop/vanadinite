// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

fn main() {
    let mut input = [0; 10];
    let mut total_read = 0;

    while total_read < 10 {
        let start = total_read;
        let read = read_stdin(&mut input[start..]);
        total_read += read;
        print!("{}", core::str::from_utf8(&input[start..][..read]).unwrap());
    }

    print!("\nyou typed: ");
    println!("{}", core::str::from_utf8(&input).unwrap());

    let result = std::syscalls::print(unsafe { core::slice::from_raw_parts(0xffffffd000004690 as *mut u8, 1024) });
    println!("{:?}", result);

    let result = std::syscalls::print(&input[..]);
    println!("\n{:?}", result);
}
