// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::VirtIoHeader;
use volatile::{Read, Volatile};

#[repr(C)]
pub struct VirtIoNetDevice {
    pub header: VirtIoHeader,
    pub mac: Volatile<[u8; 6]>,
    status: Volatile<u16, Read>,
    max_virtqueue_pairs: Volatile<u16>,
    mtu: Volatile<u16, Read>,
    speed: Volatile<u32, Read>,
    duplex: Volatile<u8, Read>,
    rss_max_key_size: Volatile<u8, Read>,
    rss_max_indirection_table_length: Volatile<u16, Read>,
    supported_hash_types: Volatile<u32, Read>,
}

impl VirtIoNetDevice {
    /// # Safety
    /// This method is only safe to call if [`NetDeviceFeatures::MULTIQUEUE`]
    /// feature has been negotiated
    pub unsafe fn link_status(&self) -> LinkStatus {
        LinkStatus(self.status.read() & 0x3)
    }

    /// # Safety
    /// This method is only safe to call if [`NetDeviceFeatures::MULTIQUEUE`]
    /// feature has been negotiated
    pub unsafe fn max_virtqueue_pairs(&self) -> &Volatile<u16> {
        &self.max_virtqueue_pairs
    }

    /// # Safety
    /// This method is only safe to call if [`NetDeviceFeatures::MAX_MTU`]
    /// feature has been negotiated
    pub unsafe fn mtu(&self) -> u16 {
        self.mtu.read()
    }

    /// # Safety
    /// This method is only safe to call if [`NetDeviceFeatures::SPEED_DUPLEX`]
    /// feature has been negotiated
    pub unsafe fn speed(&self) -> Speed {
        match self.speed.read() {
            n @ 0..=0x7FFF_FFFF => Speed::Mbps(n),
            _ => Speed::Unknown,
        }
    }

    /// # Safety
    /// This method is only safe to call if [`NetDeviceFeatures::SPEED_DUPLEX`]
    /// feature has been negotiated
    pub unsafe fn duplex(&self) -> Duplex {
        match self.duplex.read() {
            0x01 => Duplex::Full,
            0x00 => Duplex::Half,
            _ => Duplex::Unknown,
        }
    }

    /// # Safety
    /// This method is only safe to call if the [`NetDeviceFeatures::RSS`]
    /// and/or [`NetDeviceFeatures::HASH_REPORT`] features have been negotiated
    pub unsafe fn rss_max_key_size(&self) -> u8 {
        self.rss_max_key_size.read()
    }

    /// # Safety
    /// This method is only safe to call if [`NetDeviceFeatures::RSS`]
    /// feature has been negotiated
    pub unsafe fn rss_max_indirection_table_length(&self) -> u16 {
        self.rss_max_indirection_table_length.read()
    }

    /// # Safety
    /// This method is only safe to call if the [`NetDeviceFeatures::RSS`] and/or
    /// [`NetDeviceFeatures::HASH_REPORT`] features have been negotiated
    pub unsafe fn supported_hash_types(&self) -> u32 {
        self.supported_hash_types.read()
    }
}

#[derive(Clone, Copy)]
pub struct LinkStatus(u16);

impl LinkStatus {
    pub const LINK_UP: Self = Self(1);
    pub const NEED_ANNOUNCE: Self = Self(2);
}

impl core::ops::BitAnd for LinkStatus {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

impl core::fmt::Debug for LinkStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match (*self & Self::LINK_UP, *self & Self::NEED_ANNOUNCE) {
            (true, true) => core::write!(f, "LINK_UP | NEED_ANNOUNCE"),
            (false, true) => core::write!(f, "NEED_ANNOUNCE"),
            (true, false) => core::write!(f, "LINK_UP"),
            (false, false) => core::write!(f, "<NO FLAG SET>"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Duplex {
    Full,
    Half,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Speed {
    Unknown,
    Mbps(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetDeviceFeatures(u64);
pub struct NetDeviceFeaturesSplit {
    pub low: u32,
    pub high: u32,
}

impl NetDeviceFeatures {
    /// Allows offloading the checksum calculation to the device
    pub const CHKSUM_OFFLOAD: Self = Self(1 << 0);
    /// Driver handles packets with partial checksum
    pub const GUEST_CHKSUM: Self = Self(1 << 1);
    /// Control channel offloads reconfiguration support
    pub const CONTROL_GUEST_OFFLOADS: Self = Self(1 << 2);
    /// Device supports reporting the maximum MTU
    pub const MAX_MTU: Self = Self(1 << 3);
    /// Device supports reporting a valid MAC address
    pub const MAC_ADDRESS: Self = Self(1 << 5);

    /// Driver can receive TCPv4 Segment Offloaded (TSOv4) packets
    pub const GUEST_TSO4: Self = Self(1 << 7);
    /// Driver can receive TCPv6 Segment Offloaded (TSOv6) packets
    pub const GUEST_TSO6: Self = Self(1 << 8);
    /// Driver can receive TCP Segment Offloaded (TSO) packets with Explicit
    /// Congestion Notification (ECN)
    pub const GUEST_ECN: Self = Self(1 << 9);
    /// Driver can receive UDP Fragementation Offloaded (UFO) packets
    pub const GUEST_UFO: Self = Self(1 << 10);

    /// Device can receive TCPv4 Segment Offloaded (TSOv4) packets
    pub const HOST_TSO4: Self = Self(1 << 11);
    /// Device can receive TCPv6 Segment Offloaded (TSOv6) packets
    pub const HOST_TSO6: Self = Self(1 << 12);
    /// Device can receive TCP Segment Offloaded (TSO) packets with Explicit
    /// Congestion Notification (ECN)
    pub const HOST_ECN: Self = Self(1 << 13);
    /// Device can receive UDP Fragementation Offloaded (UFO) packets
    pub const HOST_UFO: Self = Self(1 << 14);

    /// Driver can merge receive buffers
    pub const MERGE_RXBUFFERS: Self = Self(1 << 15);
    /// Device supports reporting the link status
    pub const STATUS: Self = Self(1 << 16);
    pub const CONTROL_VIRTQUEUE: Self = Self(1 << 17);
    pub const CONTROL_RX_MODE: Self = Self(1 << 18);
    pub const CONTROL_VLAN_FILTERING: Self = Self(1 << 19);
    pub const GUEST_ANNOUNCE: Self = Self(1 << 21);
    pub const MULTIQUEUE: Self = Self(1 << 22);
    pub const CONTROL_MAC_ADDRESS: Self = Self(1 << 23);
    pub const HOST_USO: Self = Self(1 << 56);
    pub const HASH_REPORT: Self = Self(1 << 57);
    pub const GUEST_HEADER_LENGTH: Self = Self(1 << 59);
    pub const RSS: Self = Self(1 << 60);
    pub const RSC_EXT: Self = Self(1 << 61);
    pub const STANDBY: Self = Self(1 << 62);
    pub const SPEED_DUPLEX: Self = Self(1 << 63);

    const ALL: Self = {
        Self(
            Self::CHKSUM_OFFLOAD.0
                | Self::GUEST_CHKSUM.0
                | Self::CONTROL_GUEST_OFFLOADS.0
                | Self::MAX_MTU.0
                | Self::MAC_ADDRESS.0
                | Self::GUEST_TSO4.0
                | Self::GUEST_TSO6.0
                | Self::GUEST_ECN.0
                | Self::GUEST_UFO.0
                | Self::HOST_TSO4.0
                | Self::HOST_TSO6.0
                | Self::HOST_ECN.0
                | Self::HOST_UFO.0
                | Self::MERGE_RXBUFFERS.0
                | Self::STATUS.0
                | Self::CONTROL_VIRTQUEUE.0
                | Self::CONTROL_RX_MODE.0
                | Self::CONTROL_VLAN_FILTERING.0
                | Self::GUEST_ANNOUNCE.0
                | Self::MULTIQUEUE.0
                | Self::CONTROL_MAC_ADDRESS.0
                | Self::HOST_USO.0
                | Self::HASH_REPORT.0
                | Self::GUEST_HEADER_LENGTH.0
                | Self::RSS.0
                | Self::RSC_EXT.0
                | Self::STANDBY.0
                | Self::SPEED_DUPLEX.0,
        )
    };

    pub const fn none() -> Self {
        Self(0)
    }

    pub const fn new(raw: u64) -> Self {
        Self(raw & Self::ALL.0)
    }

    pub fn split(self) -> NetDeviceFeaturesSplit {
        NetDeviceFeaturesSplit { low: self.0 as u32, high: (self.0 >> 32) as u32 }
    }
}

impl core::ops::BitOr for NetDeviceFeatures {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        NetDeviceFeatures(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for NetDeviceFeatures {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = NetDeviceFeatures(self.0 | rhs.0);
    }
}

impl core::ops::BitAnd for NetDeviceFeatures {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct HeaderFlags(u8);

impl HeaderFlags {
    pub const NONE: Self = Self(0);
    pub const NEEDS_CHECKSUM: Self = Self(1);
    pub const DATA_VALID: Self = Self(2);
    pub const RSC_INFO: Self = Self(4);
}

impl core::ops::BitOr for HeaderFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        HeaderFlags(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for HeaderFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = HeaderFlags(self.0 | rhs.0);
    }
}

impl core::ops::BitAnd for HeaderFlags {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct GsoType(u8);

impl GsoType {
    pub const NONE: Self = Self(0);
    pub const TCPV4: Self = Self(1);
    pub const UDP: Self = Self(3);
    pub const TCPV6: Self = Self(4);
    pub const UDP_L4: Self = Self(5);
    pub const ECN: Self = Self(0x80);
}

impl core::ops::BitAnd for GsoType {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct VirtIoNetHeaderTx<const N: usize> {
    pub flags: HeaderFlags,
    pub gso_type: GsoType,
    pub header_len: u16,
    pub gso_size: u16,
    pub checksum_start: u16,
    pub checksum_offset: u16,
    pub data: [u8; N],
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct VirtIoNetHeaderRx<const N: usize> {
    pub flags: HeaderFlags,
    pub gso_type: GsoType,
    pub header_len: u16,
    pub gso_size: u16,
    pub checksum_start: u16,
    pub checksum_offset: u16,
    pub num_buffers: u16,
    pub data: [u8; N],
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct VirtIoNetHeaderTxHashReport<const N: usize> {
    pub flags: HeaderFlags,
    pub gso_type: GsoType,
    pub header_len: u16,
    pub gso_size: u16,
    pub checksum_start: u16,
    pub checksum_offset: u16,
    pub num_buffers: u16,
    pub hash_value: u32,
    pub hash_report: u16,
    pub padding_reserved: u16,
    pub data: [u8; N],
}
