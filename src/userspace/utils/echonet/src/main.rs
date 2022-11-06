// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use network::{IpV4Socket, UdpSocket};

fn main() {
    let network = std::env::lookup_capability("network").unwrap().capability.cptr;
    let mut client = UdpSocket::bind(network, IpV4Socket { ip: "0.0.0.0".parse().unwrap(), port: 1337 }).unwrap();

    let mut data = Vec::new();
    loop {
        let Ok((from, packet)) = client.recv() else { continue };
        println!("[echonet] Got: {}", core::str::from_utf8(packet).unwrap_or("<not UTF-8>").trim_end());
        data.extend_from_slice(b"you said: ");
        data.extend_from_slice(packet);
        client.send(from, &data).unwrap();
        data.clear();
    }
}
