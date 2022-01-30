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
use alchemy::PackedStruct;
use dhcp::{DhcpMessageParser, options::DhcpMessageType, DhcpOption};
use librust::{capabilities::Capability, message::KernelNotification, syscalls::ReadMessage};
use netstack::{ethernet::EthernetHeader, ipv4::{IpV4Header, Protocol, IpV4Socket, IpV4Address}, udp::UdpHeader, MacAddress, arp::{ArpPacket, HardwareType, ArpOperation, ArpHeader}};
use present::{
    ipc::{IpcChannel, NewChannelListener},
    sync::{mpsc::Sender, oneshot::OneshotTx},
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

    let (dhcp_packet_task_tx, dhcp_packet_nic_rx) = present::sync::mpsc::unbounded();
    let (dhcp_packet_nic_tx, dhcp_packet_task_rx) = present::sync::mpsc::unbounded();

    let (arp_lookup_tx, arp_lookup_rx): (Sender<(IpV4Address, OneshotTx<MacAddress>)>, _) = present::sync::mpsc::unbounded();
    let (arp_packet_task_tx, arp_packet_nic_rx): (Sender<Vec<u8>>, _) = present::sync::mpsc::unbounded();
    let (arp_packet_nic_tx, arp_packet_task_rx): (Sender<Vec<u8>>, _) = present::sync::mpsc::unbounded();

    ports.insert(68, (PortType::Udp, dhcp_packet_nic_tx));
    let this_mac = net_device.mac();

    let test_arp_sender = arp_packet_nic_tx.clone();
    present::spawn(async move {
        let mut our_ip;
        dhcp_packet_task_tx.send(dhcp_helpers::discover(this_mac));
        loop {
            let response: Vec<u8> = dhcp_packet_task_rx.recv().await;
            println!("Got response!");

            // Only accept offers back to us
            let message = match DhcpMessageParser::from_slice(&response) {
                Ok(response) => {
                    let mac: MacAddress = response.message.client_hardware_address.cast::<MacAddress>();
                    match response.message_type() {
                        Ok(DhcpMessageType::OFFER) if mac == this_mac => response,
                        _ => {
                            println!("Not offer!");
                            continue
                        }
                    }
                },
                Err(e) => {
                    println!("Error: {:?}", e);
                    continue
                }
            };

            println!("Offered IP: {}", message.your_ip_address);

            our_ip = message.your_ip_address;
            let dhcp_server_ip = match message.find_option(|option| match option {
                DhcpOption::DhcpServerIdentifier(server_ip) => Some(server_ip),
                _ => None,
            }) {
                Some(server_ip) => dbg!(server_ip),
                None => continue,
            };

            dhcp_packet_task_tx.send(dhcp_helpers::request(this_mac, dhcp_server_ip.ip(), our_ip));

            let response: Vec<u8> = dhcp_packet_task_rx.recv().await;
            match DhcpMessageParser::from_slice(&response) {
                Ok(response) => {
                    let mac: MacAddress = response.message.client_hardware_address.cast::<MacAddress>();
                    match response.message_type() {
                        Ok(DhcpMessageType::ACK) if mac == this_mac => break,
                        _ => continue,
                    }
                },
                Err(_) => continue,
            }
        }

        println!("We got an IP: {}", our_ip);

        arp::ARP_CACHE.set_lookup_task_sender(arp_lookup_tx);
        present::spawn(async move {
            let mut resolving_map: BTreeMap<IpV4Address, OneshotTx<MacAddress>> = BTreeMap::new();
            loop {
                present::select! {
                    (ip, sender) = arp_lookup_rx.recv() => {
                        let mut lookup_packet = vec![0; core::mem::size_of::<ArpPacket::<netstack::arp::Ethernet, netstack::arp::IpV4>>()];

                        let mut arp_packet = ArpPacket::<netstack::arp::Ethernet, netstack::arp::IpV4>::try_from_mut_byte_slice(&mut lookup_packet[..]).unwrap();
                        arp_packet.header = netstack::arp::ArpHeader { hardware_type: HardwareType::ETHERNET, protocol_type: netstack::arp::ProtocolType::IPV4, hardware_address_len: 6, protocol_address_len: 4, operation: ArpOperation::REQUEST };
                        arp_packet.sender_hardware_address = this_mac.bytes();
                        arp_packet.target_hardware_address = [0x00; 6];
                        arp_packet.sender_protocol_address = our_ip.to_bytes();
                        arp_packet.target_protocol_address = ip.to_bytes();

                        arp_packet_task_tx.send(lookup_packet);
                        resolving_map.insert(ip, sender);
                    }
                    packet_data = arp_packet_task_rx.recv() => {
                        if let Ok(arp_response @ ArpPacket { header: ArpHeader { operation: ArpOperation::REPLY, .. }, .. }) = ArpPacket::<netstack::arp::Ethernet, netstack::arp::IpV4>::try_from_byte_slice(&packet_data[..]) {
                            if let Some(sender) = resolving_map.remove(&IpV4Address::from(arp_response.sender_protocol_address)) {
                                sender.send(MacAddress::new(arp_response.sender_hardware_address));
                            }
                        }
                    }
                }
            }
        });

        let mac = arp::ARP_CACHE.resolve_and_cache(IpV4Address::new(10, 0, 2, 2)).await;
        println!("Resolved MAC for 10.0.2.2: {}", mac);
    });
    
    loop {
        present::select! {
            _ = interrupt.wait() => {
                println!("Interrupt");
                if let Ok(Some(packet)) = net_device.process_interrupt(interrupt_id) {
                    let (eth_header, payload, _) = EthernetHeader::split_slice_ref(packet).unwrap();
                    println!("Got packet: header = {:#?}", eth_header);
                    match eth_header.frame_type {
                        EthernetHeader::ARP_FRAME => {
                            println!("Sending ARP packet");
                            arp_packet_nic_tx.send(payload.to_vec());
                        }
                        EthernetHeader::IPV4_FRAME => {
                            println!("Got IPv4 frame");
                            let (ipv4_header, payload) = IpV4Header::split_slice_ref(payload).unwrap();
                            // TODO: verify IPv4 header checksum
                            match ipv4_header.protocol {
                                Protocol::UDP => {
                                    let (udp_header, payload) = UdpHeader::split_slice_ref(payload).unwrap();
                                    let port = udp_header.destination_port.get();
                                    if let Some((PortType::Udp, sender)) = ports.get(&port) {
                                        println!("Sending UDP packet contents to listener at port {port}");
                                        sender.send(payload.to_vec());
                                    }
                                }
                                _ => todo!(),
                            }
                        }
                        _ => {}
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
                            net_device.tx_udp4(IpV4Socket::new(todo!(), outgoing_port), (MacAddress::BROADCAST, dst_socket), &|buffer| {
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
            arp_request = arp_packet_nic_rx.recv() => {
                net_device.tx_raw(&move |bytes| {
                    let (eth_header, payload, _) = EthernetHeader::split_slice_mut(bytes).ok()?;
                    eth_header.destination_mac = MacAddress::BROADCAST;
                    eth_header.source_mac = this_mac;
                    eth_header.frame_type = EthernetHeader::ARP_FRAME;
                    payload.get_mut(..arp_request.len())?.copy_from_slice(&arp_request[..]);

                    Some(core::mem::size_of::<EthernetHeader>() + arp_request.len())
                }).unwrap();
            }
            dhcp_response = dhcp_packet_nic_rx.recv() => {
                println!("sending DHCP packet");
                net_device.tx_udp4(
                    IpV4Socket::new(
                        IpV4Address::new(0, 0, 0, 0),
                        68
                    ), 
                    (
                        MacAddress::BROADCAST,
                        IpV4Socket::new(
                            IpV4Address::new(255, 255, 255, 255),
                            67
                        )
                    ),
                    &move |buffer| {
                        buffer.get_mut(..dhcp_response.len())?.copy_from_slice(&dhcp_response);
                        Some(dhcp_response.len())
                    }
                ).unwrap();
            }
        }
    }
}

present::main!({ real_main().await });
