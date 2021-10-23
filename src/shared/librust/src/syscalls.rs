// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod allocation;
pub mod channel;
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
    SendCapability = 17,
    GrantCapability = 18,
    ReceiveCapability = 19,
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
            17 => Some(Self::SendCapability),
            18 => Some(Self::GrantCapability),
            19 => Some(Self::ReceiveCapability),
            _ => None,
        }
    }
}

#[inline]
pub fn syscall<T: Into<Message>, U: From<Message>, E: From<Message>>(
    recipient: Recipient,
    args: T,
) -> (Sender, SyscallResult<U, E>) {
    let sender: usize;
    let is_err: usize;
    let message = args.into();
    let mut t2 = message.contents[0];
    let mut t3 = message.contents[1];
    let mut t4 = message.contents[2];
    let mut t5 = message.contents[3];
    let mut t6 = message.contents[4];
    let mut a0 = message.contents[5];
    let mut a1 = message.contents[6];
    let mut a2 = message.contents[7];
    let mut a3 = message.contents[8];
    let mut a4 = message.contents[9];
    let mut a5 = message.contents[10];
    let mut a6 = message.contents[11];
    let mut a7 = message.contents[12];

    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            inlateout("t0") recipient.value() => is_err,
            inlateout("t1") 0usize => sender,
            inlateout("t2") t2,
            inlateout("t3") t3,
            inlateout("t4") t4,
            inlateout("t5") t5,
            inlateout("t6") t6,
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2,
            inlateout("a3") a3,
            inlateout("a4") a4,
            inlateout("a5") a5,
            inlateout("a6") a6,
            inlateout("a7") a7,
        );
    }

    let message = Message { contents: [t2, t3, t4, t5, t6, a0, a1, a2, a3, a4, a5, a6, a7] };
    let sender = Sender::new(sender);
    match sender.is_kernel() {
        true => match is_err == 1 {
            true => (sender, SyscallResult::Err(E::from(message))),
            false => (sender, SyscallResult::Ok(U::from(message))),
        },
        false => (sender, SyscallResult::Ok(U::from(message))),
    }
}

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
pub fn receive_message() -> Option<ReadMessage> {
    let (sender, resp) = syscall::<_, Message, ()>(
        Recipient::kernel(),
        SyscallRequest { syscall: Syscall::ReadMessage, arguments: [0; 12] },
    );

    match resp {
        SyscallResult::Ok(msg) => match sender.is_kernel() {
            true => Some(ReadMessage::Kernel(KernelNotification::from(msg))),
            false => Some(ReadMessage::User(Tid::new(sender.value().try_into().unwrap()), msg)),
        },
        SyscallResult::Err(_) => None,
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

#[inline]
pub fn claim_device(node: &str) -> SyscallResult<(*mut u8, usize), KError> {
    syscall(
        Recipient::kernel(),
        SyscallRequest {
            syscall: Syscall::ClaimDevice,
            arguments: [node.as_ptr() as usize, node.len(), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
    )
    .1
    .map(|(addr, len)| (addr as *mut _, len))
}
