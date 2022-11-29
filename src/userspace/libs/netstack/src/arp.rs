// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;

pub trait ArpHardwareTransport {
    const HARDWARE_ADDRESS_LEN: usize;
}

pub struct Ethernet;
impl ArpHardwareTransport for Ethernet {
    const HARDWARE_ADDRESS_LEN: usize = 6;
}

pub trait ArpProtocol {
    const PROTOCOL_ADDRESS_LEN: usize;
}

pub struct IpV4;
impl ArpProtocol for IpV4 {
    const PROTOCOL_ADDRESS_LEN: usize = 4;
}

#[derive(Debug)]
#[repr(C)]
pub struct ArpPacket<H: ArpHardwareTransport, P: ArpProtocol>
where
    [(); H::HARDWARE_ADDRESS_LEN]:,
    [(); P::PROTOCOL_ADDRESS_LEN]:,
{
    pub header: ArpHeader,
    pub sender_hardware_address: [u8; H::HARDWARE_ADDRESS_LEN],
    pub sender_protocol_address: [u8; P::PROTOCOL_ADDRESS_LEN],
    pub target_hardware_address: [u8; H::HARDWARE_ADDRESS_LEN],
    pub target_protocol_address: [u8; P::PROTOCOL_ADDRESS_LEN],
}

impl<H: ArpHardwareTransport, P: ArpProtocol> Copy for ArpPacket<H, P>
where
    [(); H::HARDWARE_ADDRESS_LEN]:,
    [(); P::PROTOCOL_ADDRESS_LEN]:,
{
}

impl<H: ArpHardwareTransport, P: ArpProtocol> Clone for ArpPacket<H, P>
where
    [(); H::HARDWARE_ADDRESS_LEN]:,
    [(); P::PROTOCOL_ADDRESS_LEN]:,
{
    fn clone(&self) -> Self {
        *self
    }
}

unsafe impl<H: ArpHardwareTransport, P: ArpProtocol> alchemy::OnlyValidBitPatterns for ArpPacket<H, P>
where
    [(); H::HARDWARE_ADDRESS_LEN]:,
    [(); P::PROTOCOL_ADDRESS_LEN]:,
{
}

unsafe impl<H: ArpHardwareTransport, P: ArpProtocol> alchemy::PackedStruct for ArpPacket<H, P>
where
    [(); H::HARDWARE_ADDRESS_LEN]:,
    [(); P::PROTOCOL_ADDRESS_LEN]:,
{
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
pub struct ArpHeader {
    pub hardware_type: HardwareType,
    pub protocol_type: ProtocolType,
    pub hardware_address_len: u8,
    pub protocol_address_len: u8,
    pub operation: ArpOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, PackedStruct)]
#[repr(transparent)]
pub struct HardwareType([u8; 2]);

impl HardwareType {
    pub const ETHERNET: Self = Self([0x00, 0x01]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, PackedStruct)]
#[repr(transparent)]
pub struct ProtocolType([u8; 2]);

impl ProtocolType {
    pub const IPV4: Self = Self([0x08, 0x00]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, PackedStruct)]
#[repr(transparent)]
pub struct ArpOperation([u8; 2]);

impl ArpOperation {
    pub const REQUEST: Self = Self([0x00, 0x01]);
    pub const REPLY: Self = Self([0x00, 0x02]);
}
