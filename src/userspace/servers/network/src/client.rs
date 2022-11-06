// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeMap;

use crate::{ClientMessage, ControlMessage, PortType};
use librust::capabilities::CapabilityPtr;
use netstack::ipv4::{IpV4Address, IpV4Socket};
use network::NetworkError;
use present::sync::mpsc::{Receiver, Sender};
use vidl::sync::SharedBuffer;

struct BoundPort {
    packet_rx: Receiver<ClientMessage>,
    buffer: SharedBuffer,
}

struct ClientProvider {
    control_tx: Sender<ControlMessage>,
    packet_tx: Sender<(u16, IpV4Socket, Vec<u8>)>,
    bound_ports: BTreeMap<u16, BoundPort>,
}

impl network::raw::AsyncNetworkProvider for ClientProvider {
    type Error = ();

    async fn bind_udp(
        &mut self,
        socket: network::IpV4Socket,
    ) -> Result<Result<vidl::sync::SharedBuffer, NetworkError>, Self::Error> {
        if self.bound_ports.get(&socket.port).is_some() {
            return Ok(Err(NetworkError::AlreadyBound));
        }

        let (packet_tx, packet_rx) = present::sync::mpsc::unbounded();
        self.control_tx.send(ControlMessage::NewClient { port: socket.port, port_type: PortType::Udp, tx: packet_tx });

        match packet_rx.recv().await {
            ClientMessage::PortInUse => {
                return Ok(Err(NetworkError::AlreadyBound));
            }
            ClientMessage::PortBound => {}
            msg => unreachable!("bad response message: {:?}", msg),
        }

        let buffer = SharedBuffer::new(4096).unwrap();
        let buffer2 = unsafe { buffer.clone() };

        self.bound_ports.insert(socket.port, BoundPort { packet_rx, buffer });

        Ok(Ok(buffer2))
    }

    async fn send(
        &mut self,
        socket: network::IpV4Socket,
        recipient: network::IpV4Socket,
        len: usize,
    ) -> Result<Result<(), NetworkError>, Self::Error> {
        let Some(bound_port) = self.bound_ports.get_mut(&socket.port) else { return Ok(Err(NetworkError::NotBound)) };
        self.packet_tx.send((
            socket.port,
            IpV4Socket::new(
                IpV4Address::new(
                    recipient.ip.address[0],
                    recipient.ip.address[1],
                    recipient.ip.address[2],
                    recipient.ip.address[3],
                ),
                recipient.port,
            ),
            bound_port.buffer.read()[..len].to_vec(),
        ));

        Ok(Ok(()))
    }

    async fn recv(
        &mut self,
        socket: network::IpV4Socket,
    ) -> Result<Result<network::raw::RecvInfo, NetworkError>, Self::Error> {
        let Some(bound_port) = self.bound_ports.get_mut(&socket.port) else { return Ok(Err(NetworkError::NotBound)) };
        let ClientMessage::Received { from, data } = bound_port.packet_rx.recv().await else { unreachable!() };
        // TODO: check for data size
        bound_port.buffer.copy_from_slice(&data[..]);

        Ok(Ok(network::raw::RecvInfo {
            from: network::IpV4Socket { ip: network::IpV4Address { address: from.ip.to_bytes() }, port: from.port },
            len: data.len(),
        }))
    }
}

pub async fn handle_client(
    control_tx: Sender<ControlMessage>,
    packet_tx: Sender<(u16, IpV4Socket, Vec<u8>)>,
    cptr: CapabilityPtr,
) {
    network::raw::AsyncNetwork::new(ClientProvider { control_tx, packet_tx, bound_ports: BTreeMap::new() }, cptr)
        .serve()
        .await;
}
