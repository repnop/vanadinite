// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

fn main() {
    let filesystem = std::env::lookup_capability("filesystem").unwrap().capability.cptr;
    let client = filesystem::vidl::FilesystemClient::new(filesystem);

    let mut buffer = [0u8; 128];
    let mut file = client.open("/testdir/testdir2/dirfile.txt", filesystem::vidl::OpenOptions::ReadOnly).unwrap();

    let len = file.read(&mut buffer[..]).unwrap();

    println!("[fstest] fileread = {:?}", &buffer[..len]);
}
