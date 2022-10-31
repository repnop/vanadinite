// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod raw {
    use core::str::FromStr;

    vidl::vidl_include!("network");

    impl IpV4Address {
        pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
            Self { address: [a, b, c, d] }
        }
    }

    pub struct IpV4AddressParseErr;
    impl FromStr for IpV4Address {
        type Err = IpV4AddressParseErr;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let mut parts = s.split('.');
            let a: u8 = parts.next().ok_or(IpV4AddressParseErr)?.parse().map_err(|_| IpV4AddressParseErr)?;
            let b: u8 = parts.next().ok_or(IpV4AddressParseErr)?.parse().map_err(|_| IpV4AddressParseErr)?;
            let c: u8 = parts.next().ok_or(IpV4AddressParseErr)?.parse().map_err(|_| IpV4AddressParseErr)?;
            let d: u8 = parts.next().ok_or(IpV4AddressParseErr)?.parse().map_err(|_| IpV4AddressParseErr)?;

            if parts.count() != 0 {
                return Err(IpV4AddressParseErr);
            }

            Ok(Self { address: [a, b, c, d] })
        }
    }
}

pub use raw::{IpV4Address, IpV4Socket, NetworkError};
use vidl::{sync::SharedBuffer, CapabilityPtr};

pub struct UdpSocket {
    client: raw::NetworkClient,
    buffer: SharedBuffer,
    socket: IpV4Socket,
}

impl UdpSocket {
    pub fn bind(network_cptr: CapabilityPtr, socket: IpV4Socket) -> Result<Self, NetworkError> {
        let client = raw::NetworkClient::new(network_cptr);
        let buffer = client.bind_udp(socket)?;

        Ok(Self { client, buffer, socket })
    }

    pub fn send(&mut self, recipient: IpV4Socket, data: &[u8]) -> Result<(), NetworkError> {
        let copied = self.buffer.copy_from_slice(data);
        self.client.send(recipient, copied)
    }

    pub fn recv(&mut self) -> Result<&[u8], NetworkError> {
        let len = self.client.recv(self.socket)?;
        let buf = self.buffer.read();
        Ok(&buf[..usize::min(len, buf.len())])
    }
}
