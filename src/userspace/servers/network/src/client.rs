// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{ClientMessage, ControlMessage, PortType};
use librust::capabilities::CapabilityPtr;
use netstack::ipv4::IpV4Socket;
use present::{ipc::IpcChannel, sync::mpsc::Sender};

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

pub async fn handle_client(
    control_tx: Sender<ControlMessage>,
    packet_tx: Sender<(u16, IpV4Socket, Vec<u8>)>,
    cptr: CapabilityPtr,
) {
    let mut ipc_channel = IpcChannel::new(cptr);
    let msg = ipc_channel.read(&mut []).await;

    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            println!("Error reading from IPC channel: {:?}", e);
            return;
        }
    };

    let request: BindRequest = match json::deserialize(msg.message.as_bytes()) {
        Ok(request) => request,
        Err(_) => return,
    };

    let port = request.port;
    let port_type = match &*request.port_type {
        "udp" => PortType::Udp,
        "raw" => PortType::Raw,
        _ => {
            let _ = ipc_channel
                .send_bytes(&json::to_bytes(&BindResponse { msg: String::from("unknown port type"), port: None }), &[]);
            return;
        }
    };

    let (client_tx, client_rx) = present::sync::mpsc::unbounded();
    control_tx.send(ControlMessage::NewClient { port, port_type, tx: client_tx.clone() });

    match client_rx.recv().await {
        ClientMessage::PortInUse => {
            let _ = ipc_channel
                .send_bytes(&json::to_bytes(&BindResponse { msg: String::from("port in use"), port: None }), &[]);
            return;
        }
        ClientMessage::PortBound => {
            if ipc_channel
                .send_bytes(&json::to_bytes(&BindResponse { msg: String::new(), port: Some(port) }), &[])
                .is_err()
            {
                control_tx.send(ControlMessage::ClientDisconnect { port });
                return;
            }
        }
        msg => unreachable!("bad response message: {:?}", msg),
    }

    loop {
        present::select! {
            msg = client_rx.recv() => {
                match msg {
                    ClientMessage::Received { from, data } => {
                        if ipc_channel.send_bytes(
                            &json::to_bytes(&Received {
                                from_ip: from.ip.to_string(),
                                from_port: from.port,
                                data,
                            }),
                            &[]
                        ).is_err() {
                            control_tx.send(ControlMessage::ClientDisconnect { port });
                            break;
                        }
                    },
                    _ => {}
                }
            }
            msg = ipc_channel.read(&mut []) => {
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(_) => {
                        control_tx.send(ControlMessage::ClientDisconnect { port });
                        break;
                    },
                };

                let request: SendRequest = match json::deserialize(msg.message.as_bytes()) {
                    Ok(request) => request,
                    Err(_) => {
                        control_tx.send(ControlMessage::ClientDisconnect { port });
                        break;
                    }
                };

                let ip = match request.to_ip.parse() {
                    Ok(ip) => ip,
                    Err(_) => {
                        control_tx.send(ControlMessage::ClientDisconnect { port });
                        break;
                    }
                };

                packet_tx.send((port, IpV4Socket::new(ip, request.to_port), request.data));
            }
        }
    }
}
