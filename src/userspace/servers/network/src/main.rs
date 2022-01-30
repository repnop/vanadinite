// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(const_btree_new)]

mod arp;
mod dhcp_helpers;
mod drivers;

use crate::drivers::NetworkDriver;
use librust::{capabilities::Capability, message::KernelNotification, syscalls::ReadMessage};
use netstack::{ethernet::EthernetHeader, ipv4::{IpV4Header, Protocol, IpV4Socket, IpV4Address}, udp::UdpHeader, MacAddress};
use present::{
    ipc::{IpcChannel, NewChannelListener},
    sync::mpsc::Sender,
    Present,
};
use std::collections::BTreeMap;
use sync::SpinRwLock;

json::derive! {
    #[derive(Debug, Clone)]
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    Serialize,
    struct VirtIoDeviceRequest {
        ty: u32,
    }
}

json::derive! {
    Deserialize,
    #[derive(Debug)]
    struct VirtIoDeviceResponse {
        devices: Vec<Device>,
    }
}

static ARP_CACHE: SpinRwLock<BTreeMap<IpV4Address, MacAddress>> = SpinRwLock::new(BTreeMap::new());

enum PortType {
    Udp,
    Raw,
}

async fn real_main() {
    let mut virtiomgr = IpcChannel::new(std::env::lookup_capability("virtiomgr").unwrap());

    virtiomgr
        .send_bytes(&json::to_bytes(&VirtIoDeviceRequest { ty: virtio::DeviceType::NetworkCard as u32 }), &[])
        .unwrap();

    let (message, capabilities) = virtiomgr.read_with_all_caps().await.unwrap();
    let response: VirtIoDeviceResponse = json::deserialize(message.as_bytes()).unwrap();

    if response.devices.is_empty() {
        return;
    }

    let (Capability { cptr: mmio_cap, .. }, device) = (capabilities[0], &response.devices[0]);
    let info = librust::syscalls::io::query_mmio_cap(mmio_cap).unwrap();

    let interrupt_id = device.interrupts[0];
    let mut net_device = drivers::virtio::VirtIoNetDevice::new(unsafe {
        &*(info.address() as *const virtio::devices::net::VirtIoNetDevice)
    })
    .unwrap();

    let (packet_tx, packet_recv): (Sender<(u16, IpV4Socket, Vec<u8>)>, _) = present::sync::mpsc::unbounded();
    let mut ports: BTreeMap<u16, (PortType, Sender<Vec<u8>>)> = BTreeMap::new();

    let interrupt = present::interrupt::Interrupt::new(interrupt_id);
    let channel_listener = NewChannelListener::new();

    let (dhcp_tx, dhcp_rx) = present::sync::mpsc::unbounded();
    ports.insert(68, (PortType::Udp, dhcp_tx));

    // present::spawn(dhcp_helpers::get_ip(dhcp_rx, packet_tx.clone()));

    loop {
        present::select! {
            _ = interrupt.wait() => {
                println!("interrupt happened");

                if let Ok(Some(packet)) = net_device.process_interrupt(interrupt_id) {
                    let (_eth_header, payload, _) = EthernetHeader::split_slice_ref(packet).unwrap();
                    let (ipv4_header, payload) = IpV4Header::split_slice_ref(payload).unwrap();

                    // TODO: verify IPv4 header checksum
                    match ipv4_header.protocol {
                        Protocol::UDP => {
                            let (udp_header, payload) = UdpHeader::split_slice_ref(payload).unwrap();
                            if let Some((PortType::Udp, sender)) = ports.get(&udp_header.destination_port.get()) {
                                sender.send(payload.to_vec());
                            }
                        }
                        _ => todo!(),
                    }
                }

                librust::syscalls::io::complete_interrupt(interrupt_id).unwrap();
            }
            cptr = channel_listener.recv() => {
                println!("new cptr! {:?}", cptr);
            }
            (outgoing_port, dst_socket, pkt_data) = packet_recv.recv() => {
                if let Some((port_type, _)) = ports.get(&outgoing_port) {
                    match port_type {
                        PortType::Udp => {
                            net_device.tx_udp4(IpV4Socket::new(todo!(), outgoing_port), (MacAddress::new([0xFF; 6]), dst_socket), &|buffer| {
                                if pkt_data.len() > buffer.len() {
                                    // TODO: fragment
                                    return None;
                                }

                                buffer[..pkt_data.len()].copy_from_slice(&pkt_data);
                                Some(pkt_data.len())
                            }).unwrap();
                        }
                        PortType::Raw => todo!()
                    }
                }
            }
        }
    }
}

present::main!({ real_main().await });
