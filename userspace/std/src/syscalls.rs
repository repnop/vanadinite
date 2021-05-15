// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::convert::TryFrom;

use crate::prelude::rust_2018::{AsRef, Sized};
use librust::{
    message::{Message, MessageKind, Recipient, Sender},
    syscalls::Syscall,
    task::Tid,
    KResult,
};

#[no_mangle]
pub fn syscall(recipient: Recipient, message: Message) -> Message {
    let sender: usize;
    let (mut kind_descrim, mut kind_value) = message.kind.into_parts();
    let mut fid = message.fid;
    let mut a0 = message.arguments[0];
    let mut a1 = message.arguments[1];
    let mut a2 = message.arguments[2];
    let mut a3 = message.arguments[3];
    let mut a4 = message.arguments[4];
    let mut a5 = message.arguments[5];
    let mut a6 = message.arguments[6];
    let mut a7 = message.arguments[7];

    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            inlateout("t0") recipient.value() => _,
            inlateout("t1") 0usize => sender,
            inlateout("t2") kind_descrim,
            inlateout("t3") kind_value,
            inlateout("t4") fid,
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

    Message {
        sender: Sender::new(sender),
        kind: MessageKind::from_parts(kind_descrim, kind_value).expect("kernel returned bunk message kind"),
        fid,
        arguments: [a0, a1, a2, a3, a4, a5, a6, a7],
    }
}

#[inline(always)]
pub fn exit() -> ! {
    syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::Exit as usize,
            arguments: [0; 8],
        },
    );

    unreachable!()
}

#[inline]
pub fn print<T: AsRef<[u8]> + ?Sized>(value: &T) -> KResult<()> {
    let value = value.as_ref();

    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::Print as usize,
            arguments: [value.as_ptr() as usize, value.len(), 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(drop)
}

#[inline]
pub fn read_stdin(buffer: &mut [u8]) -> KResult<usize> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::ReadStdin as usize,
            arguments: [buffer.as_ptr() as usize, buffer.len(), 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(|msg| msg.arguments[0])
}

#[inline]
pub fn receive_message() -> KResult<Option<Message>> {
    let resp = syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::ReadMessage as usize,
            arguments: [0; 8],
        },
    );

    if resp.sender == Sender::kernel() {
        Ok(None)
    } else {
        Ok(Some(resp))
    }
}

#[inline]
pub fn send_message(tid: Tid, message: Message) -> KResult<()> {
    KResult::try_from(syscall(Recipient::task(tid), message))
        .expect("bad KResult returned by kernel or something is out of sync")
        .map(drop)
}

#[inline]
pub fn alloc_memory(size_in_bytes: usize) -> KResult<*mut u8> {
    KResult::try_from(syscall(
        Recipient::kernel(),
        Message {
            sender: Sender::dummy(),
            kind: MessageKind::Request(None),
            fid: Syscall::AllocMemory as usize,
            arguments: [size_in_bytes, 0, 0, 0, 0, 0, 0, 0],
        },
    ))
    .expect("bad KResult returned by kernel or something is out of sync")
    .map(|m| m.arguments[0] as *mut u8)
}
