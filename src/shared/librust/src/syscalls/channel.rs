// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityPtr},
    syscalls::{Syscall},
};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ChannelMessage {
    pub 
}

pub fn send_message(
    cptr: CapabilityPtr,
    message: MessageId,
    message_len: usize,
    caps: &[Capability],
) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::SendChannelMessage,
            arguments: [
                cptr.value(),
                message.value(),
                message_len,
                caps.as_ptr() as usize,
                caps.len(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        },
    )
    .1
}

pub fn read_message(
    cptr: CapabilityPtr,
    cap_buffer: &mut [Capability],
) -> SyscallResult<(ChannelMessage, usize, usize), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::ReadChannel,
            arguments: [cptr.value(), cap_buffer.as_mut_ptr() as usize, cap_buffer.len(), 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|(id, ptr, len, written_caps, caps_remaining)| {
        (ChannelMessage { id: MessageId::new(id), ptr: ptr as *mut u8, len }, written_caps, caps_remaining)
    })
}

pub fn read_message_non_blocking(
    cptr: CapabilityPtr,
    cap_buffer: &mut [Capability],
) -> SyscallResult<Option<(ChannelMessage, usize, usize)>, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::ReadChannelNonBlocking,
            arguments: [cptr.value(), cap_buffer.as_mut_ptr() as usize, cap_buffer.len(), 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|vals| match vals {
        (0, 0, 0, 0, 0) => None,
        (id, ptr, len, written_caps, caps_remaining) => {
            Some((ChannelMessage { id: MessageId::new(id), ptr: ptr as *mut u8, len }, written_caps, caps_remaining))
        }
    })
}

pub fn retire_message(cptr: CapabilityPtr, message: MessageId) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::RetireChannelMessage,
            arguments: [cptr.value(), message.value(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
}
