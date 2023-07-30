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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ChannelCapability(CapabilityPtr);

impl ChannelCapability {
    pub fn new(cptr: CapabilityPtr) -> Self {
        Self(cptr)
    }

    pub fn get(self) -> usize {
        self.0.value()
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ChannelMessage(pub [usize; 7]);

#[derive(Debug)]
pub struct EndpointAlreadyMinted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub fn send(cptr: CapabilityPtr, message: ChannelMessage, caps: &[Capability]) -> Result<(), SyscallError> {
    let error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::WriteChannel as usize => error,
            in("a1") cptr.value(),
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
pub struct ReadResult {
    /// Endpoint identifier for the sender
    pub identifier: EndpointIdentifier,
    /// Message data
    pub message: ChannelMessage,
    /// Number of capabilities read
    pub capabilities_read: usize,
    /// Number of capabilities remaining which can be received by further calls
    /// to [`recv`]
    pub capabilities_remaining: usize,
}

/// Attempt to read a message and/or capabilities from an IPC channel
/// represented by the given [`CapabilityPtr`]
pub fn recv(
    cptr: CapabilityPtr,
    cap_buffer: &mut [CapabilityWithDescription],
    flags: ChannelReadFlags,
) -> Result<ReadResult, SyscallError> {
    let error: usize;
    let capabilities_read: usize;
    let capabilities_remaining: usize;
    let endpoint_id: usize;
    let mut message = [0; 7];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannel as usize => error,
            inlateout("a1") cptr.value() => capabilities_read,
            inlateout("a2") cap_buffer.as_mut_ptr() => capabilities_remaining,
            inlateout("a3") cap_buffer.len() => endpoint_id,
            in("a4") flags.0,
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
        None => Ok(ReadResult {
            identifier: EndpointIdentifier(endpoint_id),
            message: ChannelMessage(message),
            capabilities_read,
            capabilities_remaining,
        }),
    }
}

pub fn call(
    channel: ChannelCapability,
    mut message: ChannelMessage,
    to_send: &[Capability],
    to_recv: &mut [CapabilityWithDescription],
) -> Result<ReadResult, SyscallError> {
    let error: usize;
    let capabilities_read: usize;
    let capabilities_remaining: usize;
    let endpoint_id: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannel as usize => error,
            inlateout("a1") channel.get() => capabilities_read,
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
        None => Ok(ReadResult {
            identifier: EndpointIdentifier(endpoint_id),
            message,
            capabilities_read,
            capabilities_remaining,
        }),
    }
}

/// A [`CapabilityPtr`] representing the IPC channel to the kernel
pub const KERNEL_CHANNEL: CapabilityPtr = CapabilityPtr::new(0);
/// A [`CapabilityPtr`] representing the IPC channel to the parent process
pub const PARENT_CHANNEL: CapabilityPtr = CapabilityPtr::new(1);

/// See [`KernelMessage::InterruptOccurred`]
pub const KMSG_INTERRUPT_OCCURRED: usize = 0;
/// See [`KernelMessage::NewChannelMessage`]
pub const KMSG_NEW_CHANNEL_MESSAGE: usize = 1;

/// A received kernel IPC channel message
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KernelMessage {
    /// An interrupt with the given interrupt ID has occurred
    InterruptOccurred(usize),
    /// A new channel message is
    NewChannelMessage(CapabilityPtr),
}

impl KernelMessage {
    /// Turn the [`KernelMessage`] into its constituent parts
    pub const fn into_parts(self) -> [usize; 7] {
        match self {
            Self::InterruptOccurred(n) => [KMSG_INTERRUPT_OCCURRED, n, 0, 0, 0, 0, 0],
            Self::NewChannelMessage(cptr) => [KMSG_NEW_CHANNEL_MESSAGE, cptr.value(), 0, 0, 0, 0, 0],
        }
    }

    /// Constructs a new [`KernelMessage`] from the raw message parts. Panics on
    /// an invalid message type.
    pub const fn construct(parts: [usize; 7]) -> Self {
        match parts[0] {
            KMSG_INTERRUPT_OCCURRED => Self::InterruptOccurred(parts[1]),
            KMSG_NEW_CHANNEL_MESSAGE => Self::NewChannelMessage(CapabilityPtr::new(parts[1])),
            _ => unreachable!(),
        }
    }
}

impl From<KernelMessage> for [usize; 7] {
    fn from(km: KernelMessage) -> Self {
        km.into_parts()
    }
}

/// Read a [`KernelMessage`] from the kernel IPC channel
pub fn read_kernel_message() -> KernelMessage {
    KernelMessage::construct(recv(KERNEL_CHANNEL, &mut [], ChannelReadFlags::NONE).unwrap().message.0)
}
