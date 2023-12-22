// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityPtr, CapabilityWithDescription},
    error::{RawSyscallError, SyscallError},
    syscalls::Syscall,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EndpointCapability(CapabilityPtr);

impl EndpointCapability {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self(cptr)
    }

    pub fn get(self) -> CapabilityPtr {
        CapabilityPtr::new(self.0.value())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ReplyCapability(CapabilityPtr);

impl ReplyCapability {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self(cptr)
    }

    pub fn get(self) -> usize {
        self.0.value()
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
/// represented by the given [`CapabilityPtr`]
pub fn send(cptr: EndpointCapability, message: EndpointMessage, caps: &[Capability]) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::WriteChannel as usize => error,
            in("a1") cptr.get().value(),
            in("a2") caps.as_ptr(),
            in("a3") caps.len(),
            in("t0") message.0[0],
            in("t1") message.0[1],
            in("t2") message.0[2],
            in("t3") message.0[3],
            in("t4") message.0[4],
            in("t5") message.0[5],
            in("t6") message.0[6],
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

/// The result of a successful read from an IPC channel
pub struct RecvResult {
    /// Endpoint identifier for the sender
    pub identifier: EndpointIdentifier,
    /// Message data
    pub message: EndpointMessage,
    /// Number of capabilities read
    pub capabilities_read: usize,
    /// Number of capabilities remaining which can be received by further calls
    /// to [`recv`]
    pub capabilities_remaining: usize,
    // FIXME: this shouldn't exist in `call` return values
    pub reply_cptr: Option<ReplyCapability>,
}

pub enum ReadMessage {
    Kernel(KernelMessage),
    Ipc(RecvResult),
}

/// Attempt to read a message and/or capabilities from an IPC channel
/// represented by the given [`CapabilityPtr`]
pub fn recv(
    cap_buffer: &mut [CapabilityWithDescription],
    flags: ChannelReadFlags,
) -> Result<ReadMessage, SyscallError> {
    let error: usize;
    let capabilities_read: usize;
    let capabilities_remaining: usize;
    let endpoint_id: usize;
    let reply_cptr: usize;
    let mut message = [0; 7];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannel as usize => error,
            inlateout("a1") cap_buffer.as_mut_ptr() => capabilities_read,
            inlateout("a2") cap_buffer.len() => capabilities_remaining,
            inlateout("a3") flags.0 => endpoint_id,
            lateout("a4") reply_cptr,
            lateout("t0") message[0],
            lateout("t1") message[1],
            lateout("t2") message[2],
            lateout("t3") message[3],
            lateout("t4") message[4],
            lateout("t5") message[5],
            lateout("t6") message[6],
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => match endpoint_id {
            usize::MAX => Ok(ReadMessage::Kernel(KernelMessage::construct(message))),
            _ => Ok(ReadMessage::Ipc(RecvResult {
                identifier: EndpointIdentifier(endpoint_id),
                message: EndpointMessage(message),
                capabilities_read,
                capabilities_remaining,
                reply_cptr: match reply_cptr {
                    usize::MAX => None,
                    _ => Some(ReplyCapability(CapabilityPtr::new(reply_cptr))),
                },
            })),
        },
    }
}

pub fn call(
    channel: EndpointCapability,
    mut message: EndpointMessage,
    to_send: &[Capability],
    to_recv: &mut [CapabilityWithDescription],
) -> Result<RecvResult, SyscallError> {
    let error: usize;
    let capabilities_read: usize;
    let capabilities_remaining: usize;
    let endpoint_id: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannel as usize => error,
            inlateout("a1") channel.get().value() => capabilities_read,
            inlateout("a2") to_send.as_ptr() => capabilities_remaining,
            inlateout("a3") to_send.len() => endpoint_id,
            in("a4") to_recv.as_mut_ptr(),
            in("a5") to_recv.len(),
            inlateout("t0") message.0[0] => message.0[0],
            inlateout("t1") message.0[1] => message.0[1],
            inlateout("t2") message.0[2] => message.0[2],
            inlateout("t3") message.0[3] => message.0[3],
            inlateout("t4") message.0[4] => message.0[4],
            inlateout("t5") message.0[5] => message.0[5],
            inlateout("t6") message.0[6] => message.0[6],
        );
    }

    match RawSyscallError::optional(error) {
        Some(error) => Err(error.cook()),
        None => Ok(RecvResult {
            identifier: EndpointIdentifier(endpoint_id),
            message,
            capabilities_read,
            capabilities_remaining,
            reply_cptr: None,
        }),
    }
}

/// A [`CapabilityPtr`] representing the process's own IPC endpoint
pub const OWN_ENDPOINT: CapabilityPtr = CapabilityPtr::new(0);
/// A [`CapabilityPtr`] representing the parent process's IPC endpoint
pub const PARENT_CHANNEL: CapabilityPtr = CapabilityPtr::new(1);

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
