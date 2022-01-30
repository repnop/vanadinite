// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::drivers::NetworkDriver;
use alchemy::PackedStruct;
use dhcp::{
    options::{DhcpMessageType, DhcpServerIdentifier},
    DhcpMessageBuilder, DhcpOperation, DhcpOption, HardwareAddress, Seconds, TransactionId, ZeroField,
};
use netstack::{
    ipv4::{IpV4Address, IpV4Socket},
    MacAddress,
};

pub fn discover(mac: MacAddress) -> Vec<u8> {
    let mut bytes = vec![0; 1500];
    let mut dhcp_message = DhcpMessageBuilder::from_slice(&mut bytes[..]).unwrap();

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
    // dhcp_message.push_option(DhcpOption::ParameterRequestList(&[DhcpOption::DOMAIN_NAME_SERVER]));
    //dhcp_message.push_option(DhcpOption::DomainNameServer(DomainNameServerList::new(&[])));

    let len = dhcp_message.finish();
    bytes.truncate(len);

    bytes
}

pub fn request(mac: MacAddress, dhcp_server_ip: IpV4Address, our_ip: IpV4Address) -> Vec<u8> {
    let mut bytes = vec![0; 1500];
    let mut dhcp_message = DhcpMessageBuilder::from_slice(&mut bytes[..]).unwrap();

    dhcp_message.message.operation = DhcpOperation::BOOT_REQUEST;
    dhcp_message.message.hardware_address = HardwareAddress::TEN_MEGABIT_ETHERNET;
    dhcp_message.message.hardware_ops = ZeroField::new();
    dhcp_message.message.transaction_id = TransactionId::new(0);
    dhcp_message.message.secs = Seconds::new(0);
    dhcp_message.message.flags = dhcp::Flags::new(0);
    dhcp_message.message.client_ip_address = our_ip;
    dhcp_message.message.your_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.message.next_server_ip_address = dhcp_server_ip;
    dhcp_message.message.relay_agent_ip_address = IpV4Address::new(0, 0, 0, 0);
    dhcp_message.message.client_hardware_address = [0; 16];
    dhcp_message.message.client_hardware_address[..6].copy_from_slice(mac.as_bytes());
    dhcp_message.message.server_name = [0; 64];
    dhcp_message.message.boot_file_name = [0; 128];

    dhcp_message.push_option(DhcpOption::DhcpMessageType(DhcpMessageType::REQUEST));
    dhcp_message.push_option(DhcpOption::DhcpServerIdentifier(DhcpServerIdentifier::new(dhcp_server_ip)));
    let len = dhcp_message.finish();
    bytes.truncate(len);

    bytes
}
