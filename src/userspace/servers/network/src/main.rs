// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod drivers;

use alchemy::PackedStruct;
use dhcp::{
    options::DhcpMessageType, DhcpMessageBuilder, DhcpOperation, DhcpOption, HardwareAddress, Seconds, TransactionId,
    ZeroField,
};
use librust::{capabilities::Capability, message::KernelNotification, syscalls::ReadMessage};
use netstack::{
    ipv4::{IpV4Address, IpV4Socket},
    MacAddress,
};
use std::ipc::IpcChannel;

use crate::drivers::NetworkDriver;

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

    println!("Our MAC address: {}", net_device.mac_address());
    println!("Link status = {:?}", net_device.link_status());
    // println!("Max MTU = {}", net_device.max_mtu());

    let mac = net_device.mac_address();

    net_device
        .tx_udp4(
            IpV4Socket::new(IpV4Address::new(0, 0, 0, 0), 68),
            (MacAddress::new([0xFF; 6]), IpV4Socket::new(IpV4Address::new(255, 255, 255, 255), 67)),
            &|data| {
                let mut dhcp_message = DhcpMessageBuilder::from_slice(data).ok()?;

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
                dhcp_message.message.client_hardware_address[..6].copy_from_slice(mac.as_bytes());
                dhcp_message.message.server_name = [0; 64];
                dhcp_message.message.boot_file_name = [0; 128];

                dhcp_message.push_option(DhcpOption::DhcpMessageType(DhcpMessageType::DISCOVER));
                dhcp_message.push_option(DhcpOption::ParameterRequestList(&[DhcpOption::DOMAIN_NAME_SERVER]));
                //dhcp_message.push_option(DhcpOption::DomainNameServer(DomainNameServerList::new(&[])));

                Some(dhcp_message.finish())
            },
        )
        .unwrap();

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
        net_device.process_interrupt(id).unwrap();
        librust::syscalls::io::complete_interrupt(id).unwrap();
    }
}
