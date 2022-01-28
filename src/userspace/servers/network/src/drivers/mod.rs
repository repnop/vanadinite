// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use netstack::{
    ethernet::EthernetHeader,
    ipv4::{DscpEcn, Flag, FlagsFragmentOffset, Identification, IpV4Header, IpV4Socket, Protocol, VersionIhl},
    udp::{Port, UdpHeader},
    Length16, MacAddress,
};

pub mod virtio;

#[derive(Debug, Clone, Copy)]
pub enum DriverError {
    DataTooLong,
    TxQueueFull,
    RxQueueFull,
}

pub trait NetworkDriver {
    fn mac(&self) -> MacAddress;
    fn process_interrupt(&mut self, interrupt_id: usize) -> Result<Option<&[u8]>, DriverError>;
    fn tx_raw(&mut self, raw: &dyn Fn(&mut [u8]) -> Option<usize>) -> Result<(), DriverError>;

    fn tx_udp4(
        &mut self,
        source: IpV4Socket,
        destination: (MacAddress, IpV4Socket),
        data: &dyn Fn(&mut [u8]) -> Option<usize>,
    ) -> Result<(), DriverError> {
        use core::mem::size_of;

        let mac = self.mac();
        self.tx_raw(&move |buffer| {
            const HEADERS_LENGTH: usize =
                size_of::<EthernetHeader>() + size_of::<IpV4Header>() + size_of::<UdpHeader>();
            if buffer.len() < HEADERS_LENGTH {}

            let (eth_hdr, payload, _) = EthernetHeader::split_slice_mut(buffer).unwrap();
            let (ipv4_hdr, payload) = IpV4Header::split_slice_mut(payload).unwrap();
            let (udp_hdr, payload) = UdpHeader::split_slice_mut(payload).unwrap();

            let payload_size = data(payload)?;

            // Broadcast MAC
            eth_hdr.destination_mac = destination.0;
            eth_hdr.source_mac = mac;
            eth_hdr.frame_type = EthernetHeader::IPV4_FRAME;

            ipv4_hdr.version_ihl = VersionIhl::new();
            ipv4_hdr.dscp_ecn = DscpEcn::new();
            ipv4_hdr.identification = Identification::new();
            ipv4_hdr.flags_fragment_offset = FlagsFragmentOffset::new(Flag::NONE, 0);
            ipv4_hdr.ttl = 255;
            ipv4_hdr.protocol = Protocol::UDP;
            ipv4_hdr.source_ip = source.ip;
            ipv4_hdr.destination_ip = destination.1.ip;

            udp_hdr.source_port = Port::new(source.port);
            udp_hdr.destination_port = Port::new(destination.1.port);
            udp_hdr.checksum.zero();

            udp_hdr.len = Length16::new((size_of::<UdpHeader>() + payload_size) as u16);
            udp_hdr.checksum.zero();

            ipv4_hdr.len = Length16::new((size_of::<IpV4Header>() + size_of::<UdpHeader>() + payload_size) as u16);
            ipv4_hdr.generate_checksum();

            Some(HEADERS_LENGTH + payload_size)
        })
    }
}
