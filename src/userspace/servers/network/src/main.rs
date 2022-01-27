// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod drivers;

use alchemy::PackedStruct;
use dhcp::{
    options::{DhcpMessageType, DomainNameServerList},
    DhcpMessage, DhcpMessageBuilder, DhcpOperation, DhcpOption, HardwareAddress, Seconds, TransactionId, ZeroField,
};
use librust::{capabilities::Capability, message::KernelNotification, syscalls::ReadMessage};
use netstack::{
    ethernet::{EthernetHeader, Fcs},
    ipv4::{DscpEcn, Flag, FlagsFragmentOffset, Identification, IpV4Address, IpV4Header, Protocol, VersionIhl},
    udp::{Port, UdpHeader},
    Length16,
};
use std::ipc::IpcChannel;

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

fn main() {
    let mut virtiomgr = IpcChannel::new(std::env::lookup_capability("virtiomgr").unwrap());

    virtiomgr
        .send_bytes(&json::to_bytes(&VirtIoDeviceRequest { ty: virtio::DeviceType::NetworkCard as u32 }), &[])
        .unwrap();

    let (message, capabilities) = virtiomgr.read_with_all_caps().unwrap();
    let response: VirtIoDeviceResponse = json::deserialize(message.as_bytes()).unwrap();

    if response.devices.is_empty() {
        return;
    }

    let (Capability { cptr: mmio_cap, .. }, _) = (capabilities[0], &response.devices[0]);
    let info = librust::syscalls::io::query_mmio_cap(mmio_cap).unwrap();

    let mut net_device = drivers::virtio::VirtIoNetDevice::new(unsafe {
        &*(info.address() as *const virtio::devices::net::VirtIoNetDevice)
    })
    .unwrap();

    println!("Our MAC address: {}", net_device.mac_address().map(|n| format!("{:0>2X}", n)).join(":"));
    println!("Link status = {:?}", net_device.link_status());
    // println!("Max MTU = {}", net_device.max_mtu());

    let mac = net_device.mac_address();

    net_device.send_raw(|data| {
        use core::mem::size_of;

        let (eth_hdr, payload, _) = EthernetHeader::split_slice_mut(data).unwrap();
        let (ipv4_hdr, payload) = IpV4Header::split_slice_mut(payload).unwrap();
        let (udp_hdr, payload) = UdpHeader::split_slice_mut(payload).unwrap();
        let mut dhcp_message = DhcpMessageBuilder::from_slice(&mut *payload).unwrap();

        // Broadcast MAC
        eth_hdr.destination_mac = [0xFF; 6];
        eth_hdr.source_mac = mac;
        eth_hdr.frame_type = EthernetHeader::IPV4_FRAME;

        ipv4_hdr.version_ihl = VersionIhl::new();
        ipv4_hdr.dscp_ecn = DscpEcn::new();
        ipv4_hdr.identification = Identification::new();
        ipv4_hdr.flags_fragment_offset = FlagsFragmentOffset::new(Flag::NONE, 0);
        ipv4_hdr.ttl = 255;
        ipv4_hdr.protocol = Protocol::UDP;
        ipv4_hdr.source_ip = IpV4Address::new(0, 0, 0, 0);
        ipv4_hdr.destination_ip = IpV4Address::new(255, 255, 255, 255);

        udp_hdr.destination_port = Port::new(67);
        udp_hdr.source_port = Port::new(68);
        udp_hdr.checksum.zero();

        dhcp_message.message.operation = DhcpOperation::BOOT_REQUEST;
        dhcp_message.message.hardware_address = HardwareAddress::TEN_MEGABIT_ETHERNET;
        dhcp_message.message.hardware_ops = ZeroField::new();
        dhcp_message.message.transaction_id = TransactionId::new(0);
        dhcp_message.message.secs = Seconds::new(0);
        dhcp_message.message.flags = dhcp::Flags::new(0);
        dhcp_message.message.client_ip_address = IpV4Address::new(0, 0, 0, 0);
        dhcp_message.message.your_ip_address = IpV4Address::new(0, 0, 0, 0);
        dhcp_message.message.next_server_ip_address = IpV4Address::new(0, 0, 0, 0);
        dhcp_message.message.relay_agent_ip_address = IpV4Address::new(0, 0, 0, 0);
        dhcp_message.message.client_hardware_address = [0; 16];
        dhcp_message.message.client_hardware_address[..6].copy_from_slice(&mac[..]);
        dhcp_message.message.server_name = [0; 64];
        dhcp_message.message.boot_file_name = [0; 128];

        dhcp_message.push_option(DhcpOption::DhcpMessageType(DhcpMessageType::DISCOVER));
        dhcp_message.push_option(DhcpOption::ParameterRequestList(&[DhcpOption::DOMAIN_NAME_SERVER]));
        //dhcp_message.push_option(DhcpOption::DomainNameServer(DomainNameServerList::new(&[])));

        let dhcp_len = dhcp_message.finish();

        println!("dhcp_len={dhcp_len}, size_of::<DhcpMessage>()={}", size_of::<DhcpMessage>());

        udp_hdr.len = Length16::new((dhcp_len + size_of::<UdpHeader>()) as u16);
        ipv4_hdr.len = Length16::new((size_of::<IpV4Header>() + size_of::<UdpHeader>() + dhcp_len) as u16);
        ipv4_hdr.generate_checksum();

        let total_len = size_of::<EthernetHeader>() + size_of::<IpV4Header>() + size_of::<UdpHeader>() + dhcp_len;

        //let (data, fcs) = data.split_at_mut(total_len);
        //Fcs::try_from_mut_byte_slice(fcs).unwrap().generate(data);

        total_len // + 4
    });

    loop {
        let id = loop {
            match librust::syscalls::receive_message() {
                ReadMessage::Kernel(KernelNotification::InterruptOccurred(id)) => {
                    break id;
                }
                _ => continue,
            }
        };
        println!("Got interrupt!");
        net_device.recv();
        librust::syscalls::io::complete_interrupt(id).unwrap();
    }
}
