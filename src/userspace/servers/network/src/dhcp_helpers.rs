// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;
use dhcp::{
    options::DhcpMessageType, DhcpMessageBuilder, DhcpOperation, DhcpOption, HardwareAddress, Seconds, TransactionId,
    ZeroField,
};
use netstack::{ipv4::IpV4Address, MacAddress};

pub fn discover(mac: MacAddress) -> Vec<u8> {
    let mut bytes = vec![0; 1500];
    let mut dhcp_message = DhcpMessageBuilder::from_slice(&mut bytes[..]).unwrap();

    dhcp_message.operation = DhcpOperation::BOOT_REQUEST;
    dhcp_message.hardware_address = HardwareAddress::TEN_MEGABIT_ETHERNET;
    dhcp_message.hardware_ops = ZeroField::new();
    dhcp_message.transaction_id = TransactionId::new(0);
    dhcp_message.secs = Seconds::new(0);
    dhcp_message.flags = dhcp::Flags::new(0);
    dhcp_message.client_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.your_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.next_server_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.relay_agent_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.client_hardware_address = [0; 16];
    dhcp_message.client_hardware_address[..6].copy_from_slice(mac.as_bytes());
    dhcp_message.server_name = [0; 64];
    dhcp_message.boot_file_name = [0; 128];

    dhcp_message.push_option(DhcpOption::DhcpMessageType(DhcpMessageType::DISCOVER));
    dhcp_message.push_option(DhcpOption::ParameterRequestList(&[DhcpOption::ROUTER]));

    let len = dhcp_message.finish();
    bytes.truncate(len);

    bytes
}

pub fn request(mac: MacAddress, dhcp_server_ip: IpV4Address, our_ip: IpV4Address) -> Vec<u8> {
    let mut bytes = vec![0; 1500];
    let mut dhcp_message = DhcpMessageBuilder::from_slice(&mut bytes[..]).unwrap();

    dhcp_message.operation = DhcpOperation::BOOT_REQUEST;
    dhcp_message.hardware_address = HardwareAddress::TEN_MEGABIT_ETHERNET;
    dhcp_message.hardware_ops = ZeroField::new();
    dhcp_message.transaction_id = TransactionId::new(0);
    dhcp_message.secs = Seconds::new(0);
    dhcp_message.flags = dhcp::Flags::new(0);
    dhcp_message.client_ip_address = our_ip;
    dhcp_message.your_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.next_server_ip_address = dhcp_server_ip;
    dhcp_message.relay_agent_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.client_hardware_address = [0; 16];
    dhcp_message.client_hardware_address[..6].copy_from_slice(mac.as_bytes());
    dhcp_message.server_name = [0; 64];
    dhcp_message.boot_file_name = [0; 128];

    dhcp_message.push_option(DhcpOption::DhcpMessageType(DhcpMessageType::REQUEST));
    dhcp_message.push_option(DhcpOption::DhcpServerIdentifier(dhcp_server_ip));
    let len = dhcp_message.finish();
    bytes.truncate(len);

    bytes
}
