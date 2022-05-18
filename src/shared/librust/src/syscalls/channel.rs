// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityPtr},
    error::{RawSyscallError, SyscallError},
    syscalls::Syscall,
};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ChannelMessage(pub [usize; 7]);

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

pub fn send_message(cptr: CapabilityPtr, message: ChannelMessage, caps: &[Capability]) -> Result<(), SyscallError> {
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

pub fn read_message(
    cptr: CapabilityPtr,
    cap_buffer: &mut [Capability],
    flags: ChannelReadFlags,
) -> Result<(ChannelMessage, usize, usize), SyscallError> {
    let error: usize;
    let read_caps: usize;
    let remaining_caps: usize;
    let mut message = [0; 7];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannel as usize => error,
            inlateout("a1") cptr.value() => read_caps,
            inlateout("a2") cap_buffer.as_mut_ptr() => remaining_caps,
            in("a3") cap_buffer.len(),
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
        None => Ok((ChannelMessage(message), read_caps, remaining_caps)),
    }
}

pub const KERNEL_CHANNEL: CapabilityPtr = CapabilityPtr::new(0);
pub const PARENT_CHANNEL: CapabilityPtr = CapabilityPtr::new(1);

pub const KMSG_INTERRUPT_OCCURRED: usize = 0;
pub const KMSG_NEW_CHANNEL_MESSAGE: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KernelMessage {
    InterruptOccurred(usize),
    NewChannelMessage(CapabilityPtr),
}

impl KernelMessage {
    pub const fn into_parts(self) -> [usize; 7] {
        match self {
            Self::InterruptOccurred(n) => [
                KMSG_INTERRUPT_OCCURRED,
                n,
                0,
                0, 0, 0, 0
            ],
            Self::NewChannelMessage(cptr) => [
                KMSG_NEW_CHANNEL_MESSAGE,
                cptr.value(),
                0,
                0, 0, 0, 0
            ],
        }
    }

    pub const fn construct(parts: [usize; 7]) -> Self {
        match parts[0] {
            KMSG_INTERRUPT_OCCURRED => Self::InterruptOccurred(parts[1]),
            KMSG_NEW_CHANNEL_MESSAGE => Self::NewChannelMessage(CapabilityPtr::new(parts[1])),
            _ => unreachable!(),
        }
    }
}

pub fn read_kernel_message() -> KernelMessage {
    KernelMessage::construct(read_message(KERNEL_CHANNEL, &mut [], ChannelReadFlags::NONE).unwrap().0.0)
}