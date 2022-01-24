// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

alchemy::derive! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct DhcpMessageType(pub u8);
}

impl DhcpMessageType {
    pub const DISCOVER: Self = Self(1);
    pub const OFFER: Self = Self(2);
    pub const REQUEST: Self = Self(3);
    pub const DECLINE: Self = Self(4);
    pub const ACK: Self = Self(5);
    pub const NAK: Self = Self(6);
    pub const RELEASE: Self = Self(7);
    pub const INFORM: Self = Self(8);
    pub const FORCE_RENEW: Self = Self(9);
    pub const LEASE_QUERY: Self = Self(10);
    pub const LEASE_UNASSIGNED: Self = Self(11);
    pub const LEASE_UNKNOWN: Self = Self(12);
    pub const LEASE_ACTIVE: Self = Self(13);
    pub const BULK_LEASE_QUERY: Self = Self(14);
    pub const LEASE_QUERY_DONE: Self = Self(15);
    pub const ACTIVE_LEASE_QUERY: Self = Self(16);
    pub const LEASE_QUERY_STATUS: Self = Self(17);
    pub const TLS: Self = Self(18);

    pub fn new(message_type: u8) -> Self {
        Self(message_type)
    }
}

pub struct DomainNameServerList<'a>(pub(crate) &'a [super::IpV4Address]);

impl<'a> DomainNameServerList<'a> {
    pub fn new(servers: &'a [super::IpV4Address]) -> Self {
        Self(servers)
    }

    pub fn servers(&self) -> impl Iterator<Item = super::IpV4Address> + 'a {
        self.0.iter().copied()
    }
}
