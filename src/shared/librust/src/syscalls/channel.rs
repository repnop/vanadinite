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

pub fn send_message(cptr: CapabilityPtr, message: ChannelMessage, caps: &[Capability]) -> Result<(), SyscallError> {
    let error: Option<RawSyscallError>;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::SendChannelMessage => error,
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

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

pub fn read_message(
    cptr: CapabilityPtr,
    cap_buffer: &mut [Capability],
) -> Result<(ChannelMessage, usize, usize), SyscallError> {
    let error: Option<RawSyscallError>;
    let read_caps: usize;
    let remaining_caps: usize;
    let message = [0; 7];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannel => error,
            inlateout("a1") cptr.value() => read_caps,
            inlateout("a2") cap_buffer.as_mut_ptr() => remaining_caps,
            in("a3") cap_buffer.len(),
            lateout("t0") message[0],
            lateout("t1") message[1],
            lateout("t2") message[2],
            lateout("t3") message[3],
            lateout("t4") message[4],
            lateout("t5") message[5],
            lateout("t6") message[6],
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok((ChannelMessage(message), read_caps, remaining_caps)),
    }
}

pub fn read_message_non_blocking(
    cptr: CapabilityPtr,
    cap_buffer: &mut [Capability],
) -> Result<Option<(ChannelMessage, usize, usize)>, SyscallError> {
    let error: Option<RawSyscallError>;
    let read_caps: usize;
    let remaining_caps: usize;
    let message = [0; 7];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::ReadChannelNonBlocking => error,
            inlateout("a1") cptr.value() => read_caps,
            inlateout("a2") cap_buffer.as_mut_ptr() => remaining_caps,
            in("a3") cap_buffer.len(),
            lateout("t0") message[0],
            lateout("t1") message[1],
            lateout("t2") message[2],
            lateout("t3") message[3],
            lateout("t4") message[4],
            lateout("t5") message[5],
            lateout("t6") message[6],
        );
    }

    match error {
        Some(error) => match error.cook() {
            SyscallError::WouldBlock => Ok(None),
            error => Err(error),
        },
        None => Ok(Some((ChannelMessage(message), read_caps, remaining_caps))),
    }
}
