// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityId, CapabilityPtr, CapabilityRights, CapabilityType},
    error::{RawSyscallError, SyscallError},
    syscalls::Syscall,
    Either,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EndpointCapability(CapabilityPtr);

impl EndpointCapability {
    pub const fn new(cptr: CapabilityPtr) -> Self {
        Self(cptr)
    }

    pub const fn get(self) -> CapabilityPtr {
        CapabilityPtr::from_raw(self.0.value())
    }

    pub fn send(self, message: EndpointMessage, capability_to_send: Option<Capability>) -> Result<(), SyscallError> {
        send(self, message, capability_to_send)
    }

    pub fn send_with_reply(
        self,
        message: EndpointMessage,
        capability_to_send: Option<Capability>,
    ) -> Result<ReplyId, SyscallError> {
        send_with_reply(self, message, capability_to_send)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ReplyCapability(CapabilityPtr);

impl ReplyCapability {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self(cptr)
    }

    pub fn get(self) -> CapabilityPtr {
        CapabilityPtr::from_raw(self.0.value())
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct EndpointMessage(pub [usize; 7]);

#[derive(Debug)]
pub struct EndpointAlreadyMinted;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EndpointIdentifier(usize);

impl EndpointIdentifier {
    pub const UNIDENTIFIED: Self = Self(0);

    pub fn new(raw: usize) -> Self {
        Self(raw)
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ReplyId(u64);

impl ReplyId {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(transparent)]
pub struct ChannelReadFlags(usize);

impl ChannelReadFlags {
    pub const NONE: Self = Self(0);
    pub const NONBLOCKING: Self = Self(1);

    pub const fn new(flags: usize) -> Self {
        Self(flags)
    }

    pub const fn value(self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for ChannelReadFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for ChannelReadFlags {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.0 & rhs.0 == rhs.0
    }
}

/// Attempt to send a message and/or capabilitires on the IPC channel
/// represented by the given [`EndpointCapability`]
pub fn send(
    endpoint: EndpointCapability,
    message: EndpointMessage,
    capability_to_send: Option<Capability>,
) -> Result<(), SyscallError> {
    let error: usize;
    let (cptr, rights) = capability_to_send.map(|c| (c.cptr, c.rights)).unwrap_or((
        CapabilityPtr::from_raw_parts(
            CapabilityId::from_raw(usize::MAX << 4),
            crate::capabilities::CapabilityType::Endpoint,
        ),
        CapabilityRights::NONE,
    ));

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::Send as usize => error,
            in("a1") endpoint.get().value(),
            in("a2") cptr.value(),
            in("a3") rights.value(),
            // Reply endpoint: No
            in("a4") 0,
            in("t0") message.0[0],
            in("t1") message.0[1],
            in("t2") message.0[2],
            in("t3") message.0[3],
            in("t4") message.0[4],
            in("t5") message.0[5],
            in("t6") message.0[6],
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

/// Attempt to send a message and/or capabilitires on the IPC channel
/// represented by the given [`EndpointCapability`]
pub fn send_with_reply(
    endpoint: EndpointCapability,
    message: EndpointMessage,
    capability_to_send: Option<Capability>,
) -> Result<ReplyId, SyscallError> {
    let error: usize;
    let reply_id: u64;
    let (cptr, rights) = capability_to_send.map(|c| (c.cptr, c.rights)).unwrap_or((
        CapabilityPtr::from_raw_parts(
            CapabilityId::from_raw(usize::MAX << 4),
            crate::capabilities::CapabilityType::Endpoint,
        ),
        CapabilityRights::NONE,
    ));

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::Send as usize => error,
            inlateout("a1") endpoint.get().value() => reply_id,
            in("a2") cptr.value(),
            in("a3") rights.value(),
            // Reply endpoint: Yes
            in("a4") 1,
            in("t0") message.0[0],
            in("t1") message.0[1],
            in("t2") message.0[2],
            in("t3") message.0[3],
            in("t4") message.0[4],
            in("t5") message.0[5],
            in("t6") message.0[6],
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok(ReplyId(reply_id)),
    }
}

///
pub struct IpcMessage {
    /// Endpoint identifier of the sender
    pub identifier: EndpointIdentifier,
    /// Message data
    pub message: EndpointMessage,
    pub capability: Option<Capability>,
    pub reply: Option<Either<ReplyCapability, ReplyId>>,
}

pub enum Message {
    Kernel(KernelMessage),
    Ipc(IpcMessage),
}

pub const RECV_NO_REPLY_INFO: usize = 0;
pub const RECV_REPLY_ENDPOINT: usize = 1;
pub const RECV_REPLY_ID: usize = 2;

/// Attempt to read a message and/or capabilities from an IPC channel
/// represented by the given [`CapabilityPtr`]
pub fn recv(flags: ChannelReadFlags) -> Result<Message, SyscallError> {
    let error: usize;
    let endpoint_id: usize;
    let sent_cap: usize;
    let sent_cap_rights: usize;
    let reply_value: usize;
    let reply_type: usize;
    let mut message = [0; 7];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::Recv as usize => error,
            inlateout("a1") flags.0 => endpoint_id,
            lateout("a2") sent_cap,
            lateout("a3") sent_cap_rights,
            lateout("a4") reply_value,
            lateout("a5") reply_type,
            lateout("t0") message[0],
            lateout("t1") message[1],
            lateout("t2") message[2],
            lateout("t3") message[3],
            lateout("t4") message[4],
            lateout("t5") message[5],
            lateout("t6") message[6],
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => match endpoint_id {
            usize::MAX => Ok(Message::Kernel(KernelMessage::construct(message))),
            _ => Ok(Message::Ipc(IpcMessage {
                identifier: EndpointIdentifier(endpoint_id),
                message: EndpointMessage(message),
                capability: match CapabilityRights::new(sent_cap_rights) {
                    CapabilityRights::NONE => None,
                    _ => Some(Capability {
                        cptr: CapabilityPtr::from_raw(sent_cap),
                        rights: CapabilityRights::new(sent_cap_rights),
                    }),
                },
                reply: match reply_type {
                    RECV_NO_REPLY_INFO => None,
                    RECV_REPLY_ENDPOINT => Some(Either::Left(ReplyCapability(CapabilityPtr::from_raw(reply_value)))),
                    RECV_REPLY_ID => Some(Either::Right(ReplyId(reply_value as u64))),
                    _ => unreachable!("bad kernel reply_type"),
                },
            })),
        },
    }
}

pub fn call(
    endpoint: EndpointCapability,
    mut message: EndpointMessage,
    to_send: Option<Capability>,
) -> Result<(EndpointMessage, Option<Capability>), SyscallError> {
    let error: usize;
    let received_cap: usize;
    let received_cap_rights: usize;

    let (sending_cptr, sending_rights) =
        to_send.map(|c| (c.cptr.value(), c.rights.value())).unwrap_or((usize::MAX, CapabilityRights::NONE.value()));

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::Recv as usize => error,
            inlateout("a1") endpoint.get().value() => received_cap,
            inlateout("a2") sending_cptr => received_cap_rights,
            in("a3") sending_rights,
            inlateout("t0") message.0[0] => message.0[0],
            inlateout("t1") message.0[1] => message.0[1],
            inlateout("t2") message.0[2] => message.0[2],
            inlateout("t3") message.0[3] => message.0[3],
            inlateout("t4") message.0[4] => message.0[4],
            inlateout("t5") message.0[5] => message.0[5],
            inlateout("t6") message.0[6] => message.0[6],
        );
    }

    match RawSyscallError::from_raw(error) {
        Some(error) => Err(error.cook()),
        None => Ok((
            message,
            match CapabilityRights::new(received_cap_rights) {
                CapabilityRights::NONE => None,
                rights => Some(Capability::new(CapabilityPtr::from_raw(received_cap), rights)),
            },
        )),
    }
}

/// A [`EndpointCapability`] representing the process's own IPC endpoint
pub const OWN_ENDPOINT: EndpointCapability =
    EndpointCapability::new(CapabilityPtr::from_raw_parts(CapabilityId::from_raw(0), CapabilityType::Endpoint));
/// A [`EndpointCapability`] representing the parent process's IPC endpoint
pub const PARENT_CHANNEL: EndpointCapability =
    EndpointCapability::new(CapabilityPtr::from_raw_parts(CapabilityId::from_raw(1), CapabilityType::Endpoint));

/// See [`KernelMessage::InterruptOccurred`]
pub const KMSG_INTERRUPT_OCCURRED: usize = 0;

/// A received kernel IPC channel message
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KernelMessage {
    /// An interrupt with the given interrupt ID has occurred
    InterruptOccurred(usize),
}

impl KernelMessage {
    /// Turn the [`KernelMessage`] into its constituent parts
    pub const fn into_parts(self) -> [usize; 7] {
        match self {
            Self::InterruptOccurred(n) => [KMSG_INTERRUPT_OCCURRED, n, 0, 0, 0, 0, 0],
        }
    }

    /// Constructs a new [`KernelMessage`] from the raw message parts. Panics on
    /// an invalid message type.
    pub const fn construct(parts: [usize; 7]) -> Self {
        match parts[0] {
            KMSG_INTERRUPT_OCCURRED => Self::InterruptOccurred(parts[1]),
            _ => unreachable!(),
        }
    }
}

impl From<KernelMessage> for [usize; 7] {
    fn from(km: KernelMessage) -> Self {
        km.into_parts()
    }
}
