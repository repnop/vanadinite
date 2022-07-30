// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ipc::{ChannelMessage, ChannelReadFlags, IpcChannel};

json::derive! {
    #[derive(Debug, Clone)]
    struct BindRequest {
        port: u16,
        port_type: String,
    }
}

json::derive! {
    #[derive(Debug, Clone)]
    struct BindResponse {
        msg: String,
        port: Option<u16>,
    }
}

json::derive! {
    #[derive(Debug, Clone)]
    struct SendRequest {
        // FIXME: this should be an IpV4Socket
        to_ip: String,
        to_port: u16,
        data: Vec<u8>,
    }
}

json::derive! {
    #[derive(Debug, Clone)]
    struct SendResponse {
        msg: String,
        ok: bool,
    }
}

json::derive! {
    #[derive(Debug, Clone)]
    struct Received {
        // FIXME: this should be an IpV4Socket
        from_ip: String,
        from_port: u16,
        data: Vec<u8>,
    }
}

fn main() {
    let network = IpcChannel::new(std::env::lookup_capability("network").unwrap().capability.cptr);
    network
        .temp_send_json(ChannelMessage::default(), &BindRequest { port: 1337, port_type: String::from("udp") }, &[])
        .unwrap();
    let (bind_response, _, _): (BindResponse, _, _) = network.temp_read_json(ChannelReadFlags::NONE).unwrap();
    match bind_response.port {
        Some(port) => println!("Bound to port {}", port),
        None => {
            println!("Couldn't bind to port 1337: {}", bind_response.msg);
            return;
        }
    }

    loop {
        let (received, _, _): (Received, _, _) = network.temp_read_json(ChannelReadFlags::NONE).unwrap();
        println!("Got message, replying!");
        network
            .temp_send_json(
                ChannelMessage::default(),
                &SendRequest {
                    to_port: received.from_port,
                    to_ip: received.from_ip,
                    data: (*b"you said: ").into_iter().chain(received.data).collect(),
                },
                &[],
            )
            .unwrap();
    }
}
