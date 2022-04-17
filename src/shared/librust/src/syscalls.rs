// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod allocation;
pub mod channel;
pub mod io;
pub mod mem;
pub mod vmspace;

use crate::{
    error::KError,
    message::{KernelNotification, Message, Recipient, Sender, SyscallRequest, SyscallResult},
    task::Tid,
};
use core::{convert::TryInto, num::NonZeroUsize};

#[derive(Debug)]
#[repr(usize)]
pub enum Syscall {
    Exit = 0,
    Print = 1,
    ReadStdin = 2,
    ReadMessage = 3,
    AllocVirtualMemory = 4,
    GetTid = 5,
    ReadChannel = 7,
    CreateChannelMessage = 8,
    SendChannelMessage = 9,
    RetireChannelMessage = 10,
    AllocDmaMemory = 12,
    CreateVmspace = 13,
    AllocVmspaceObject = 14,
    SpawnVmspace = 15,
    ClaimDevice = 16,
    QueryMemoryCapability = 20,
    CompleteInterrupt = 21,
    QueryMmioCapability = 22,
    ReadChannelNonBlocking = 23,
}

impl Syscall {
    pub fn from_usize(n: usize) -> Option<Self> {
        match n {
            0 => Some(Self::Exit),
            1 => Some(Self::Print),
            2 => Some(Self::ReadStdin),
            3 => Some(Self::ReadMessage),
            4 => Some(Self::AllocVirtualMemory),
            5 => Some(Self::GetTid),
            7 => Some(Self::ReadChannel),
            8 => Some(Self::CreateChannelMessage),
            9 => Some(Self::SendChannelMessage),
            10 => Some(Self::RetireChannelMessage),
            12 => Some(Self::AllocDmaMemory),
            13 => Some(Self::CreateVmspace),
            14 => Some(Self::AllocVmspaceObject),
            15 => Some(Self::SpawnVmspace),
            16 => Some(Self::ClaimDevice),
            20 => Some(Self::QueryMemoryCapability),
            21 => Some(Self::CompleteInterrupt),
            22 => Some(Self::QueryMmioCapability),
            23 => Some(Self::ReadChannelNonBlocking),
            _ => None,
        }
    }
}

pub unsafe fn syscall0(id: Syscall) -> Result<

#[inline(always)]
pub fn exit() -> ! {
    let _ = syscall::<_, (), ()>(Recipient::kernel(), SyscallRequest { syscall: Syscall::Exit, arguments: [0; 12] });

    unreachable!()
}

#[inline]
pub fn print(value: &[u8]) -> SyscallResult<(), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::Print,
            arguments: [value.as_ptr() as usize, value.len(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
}

#[inline]
pub fn read_stdin(buffer: &mut [u8]) -> SyscallResult<usize, KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::ReadStdin,
            arguments: [buffer.as_ptr() as usize, buffer.len(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
}

#[derive(Debug, Clone, Copy)]
pub enum ReadMessage {
    Kernel(KernelNotification),
    User(Tid, Message),
}

#[inline]
pub fn receive_message() -> ReadMessage {
    let (sender, resp) = syscall::<_, Message, ()>(
        Recipient::kernel(),
        SyscallRequest { syscall: Syscall::ReadMessage, arguments: [0; 12] },
    );

    match resp {
        SyscallResult::Ok(msg) => match sender.is_kernel() {
            true => ReadMessage::Kernel(KernelNotification::from(msg)),
            false => ReadMessage::User(Tid::new(sender.value().try_into().unwrap()), msg),
        },
        SyscallResult::Err(_) => unreachable!(),
    }
}

#[inline]
pub fn send_message(tid: Tid, message: Message) -> SyscallResult<(), KError> {
    syscall(Recipient::task(tid), message).1
}

#[inline]
pub fn current_tid() -> Tid {
    Tid::new(
        NonZeroUsize::new(
            syscall::<_, (usize,), ()>(
                Recipient::kernel(),
                SyscallRequest { syscall: Syscall::GetTid, arguments: [0; 12] },
            )
            .1
            .unwrap()
            .0,
        )
        .unwrap(),
    )
}
