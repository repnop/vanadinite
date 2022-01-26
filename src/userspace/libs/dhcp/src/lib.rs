// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(split_array)]

pub mod options;

use alchemy::PackedStruct;
use netstack::ipv4::IpV4Address;

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct DhcpMessage {
        pub operation: DhcpOperation,
        pub hardware_address: HardwareAddress,
        pub hardware_ops: ZeroField,
        pub transaction_id: TransactionId,
        pub secs: Seconds,
        pub flags: Flags,
        pub client_ip_address: IpV4Address,
        pub your_ip_address: IpV4Address,
        pub next_server_ip_address: IpV4Address,
        pub relay_agent_ip_address: IpV4Address,
        pub client_hardware_address: [u8; 16],
        pub server_name: [u8; 64],
        pub boot_file_name: [u8; 128],
        pub magic_cookie: MagicCookie,
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct DhcpOperation(u8);
}

impl DhcpOperation {
    pub const BOOT_REQUEST: Self = Self(1);
    pub const BOOT_REPLY: Self = Self(2);
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(C)]
    pub struct HardwareAddress {
        r#type: u8,
        len: u8,
    }
}

impl HardwareAddress {
    pub const TEN_MEGABIT_ETHERNET: Self = Self { r#type: 1, len: 6 };

    pub fn new(r#type: u8, len: u8) -> Self {
        Self { r#type, len }
    }

    pub fn hw_addr_type(self) -> u8 {
        self.r#type
    }

    pub fn hw_addr_len(self) -> u8 {
        self.len
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct ZeroField(u8);
}

impl ZeroField {
    pub fn new() -> Self {
        Self(0)
    }
}

impl Default for ZeroField {
    fn default() -> Self {
        Self::new()
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct TransactionId([u8; 4]);
}

impl TransactionId {
    pub fn new(n: u32) -> Self {
        Self(n.to_be_bytes())
    }

    pub fn get(self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub struct Seconds([u8; 2]);
}

impl Seconds {
    pub fn new(n: u16) -> Self {
        Self(n.to_be_bytes())
    }

    pub fn get(self) -> u16 {
        u16::from_be_bytes(self.0)
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub struct Flags([u8; 2]);
}

impl Flags {
    pub fn new(n: u16) -> Self {
        Self(n.to_be_bytes())
    }

    pub fn get(self) -> u16 {
        u16::from_be_bytes(self.0)
    }
}

alchemy::derive! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct MagicCookie([u8; 4]);
}

impl MagicCookie {
    pub fn new() -> Self {
        Self([0x63, 0x82, 0x53, 0x63])
    }

    pub fn is_valid(self) -> bool {
        self.0 == [0x63, 0x82, 0x53, 0x63]
    }
}

impl Default for MagicCookie {
    fn default() -> Self {
        Self::new()
    }
}

pub enum DhcpOption<'a> {
    DomainNameServer(options::DomainNameServerList<'a>),
    DhcpMessageType(options::DhcpMessageType),
    EndOfOptions,
}

impl DhcpOption<'_> {
    pub fn option_id(&self) -> u8 {
        match self {
            DhcpOption::DomainNameServer(_) => 6,
            Self::DhcpMessageType(_) => 53,
            Self::EndOfOptions => 255,
        }
    }
}

pub struct DhcpMessageBuilder<'a> {
    pub message: &'a mut DhcpMessage,
    options: &'a mut [u8],
    used_space: usize,
}

impl<'a> DhcpMessageBuilder<'a> {
    pub fn from_slice(slice: &'a mut [u8]) -> Result<Self, alchemy::TryCastError> {
        if slice.len() < core::mem::size_of::<DhcpMessage>() {
            return Err(alchemy::TryCastError::NotLongEnough);
        }

        let (message, options) = slice.split_array_mut::<{ core::mem::size_of::<DhcpMessage>() }>();

        Ok(Self {
            message: DhcpMessage::from_bytes_mut::<{ core::mem::size_of::<DhcpMessage>() }>(message),
            options,
            used_space: core::mem::size_of::<DhcpMessage>(),
        })
    }

    pub fn from_array<const N: usize>(slice: &'a mut [u8; N]) -> Result<Self, alchemy::TryCastError> {
        if slice.len() < core::mem::size_of::<DhcpMessage>() {
            return Err(alchemy::TryCastError::NotLongEnough);
        }

        let (message, options) = slice.split_array_mut::<{ core::mem::size_of::<DhcpMessage>() }>();

        Ok(Self {
            message: DhcpMessage::from_bytes_mut::<{ core::mem::size_of::<DhcpMessage>() }>(message),
            options,
            used_space: core::mem::size_of::<DhcpMessage>(),
        })
    }

    pub fn try_push_option(&mut self, option: DhcpOption<'_>) -> Result<(), TryPushOptionError> {
        let option_id = option.option_id();
        match option {
            DhcpOption::DomainNameServer(servers) => {
                let servers_len =
                    u8::try_from(servers.0.len() * 4).map_err(|_| TryPushOptionError::OptionValueTooLong)?;
                self.push_bytes(&[option_id, servers_len])?;
                self.push_bytes(IpV4Address::bytes_of_slice(servers.0))?;
            }
            DhcpOption::DhcpMessageType(mtype) => {
                self.push_bytes(&[option_id, 1, mtype.0])?;
            }
            DhcpOption::EndOfOptions => {
                self.push_bytes(&[option_id])?;
            }
        }

        Ok(())
    }

    pub fn push_option(&mut self, option: DhcpOption<'_>) {
        self.try_push_option(option).expect("failed to push DHCP option")
    }

    #[must_use]
    pub fn finish(mut self) -> usize {
        self.push_option(DhcpOption::EndOfOptions);

        self.message.magic_cookie = MagicCookie::new();
        self.message.hardware_ops = ZeroField::new();

        self.used_space
    }

    fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), TryPushOptionError> {
        if self.options.len() < bytes.len() {
            return Err(TryPushOptionError::BufferTooShort);
        }

        let (pushing, rest) = core::mem::take(&mut self.options).split_at_mut(bytes.len());
        pushing.copy_from_slice(bytes);

        self.options = rest;
        self.used_space += bytes.len();

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TryPushOptionError {
    BufferTooShort,
    OptionValueTooLong,
}
