// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::message::{Message, MessageKind};
use core::convert::TryFrom;

pub(crate) const INVALID_ACCESS: usize = 1;
pub(crate) const INVALID_MESSAGE: usize = 2;
pub(crate) const INVALID_RECIPIENT: usize = 3;
pub(crate) const INVALID_SYSCALL: usize = 4;

#[derive(Debug)]
pub enum KError {
    InvalidAccess(AccessError),
    InvalidMessage,
    InvalidRecipient,
    InvalidSyscall(usize),
}

impl TryFrom<Message> for KError {
    type Error = &'static str;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        match msg.kind {
            MessageKind::Reply(Some(err)) => match err.get() {
                const { INVALID_MESSAGE } => Ok(Self::InvalidMessage),
                const { INVALID_RECIPIENT } => Ok(Self::InvalidRecipient),
                const { INVALID_SYSCALL } => Ok(Self::InvalidSyscall(msg.arguments[0])),
                const { INVALID_ACCESS } => Ok(Self::InvalidAccess(match msg.arguments[0] {
                    0 => AccessError::Read(msg.arguments[1] as _),
                    1 => AccessError::Write(msg.arguments[1] as _),
                    _ => return Err("invalid error cause for access error"),
                })),
                _ => unreachable!(),
            },
            MessageKind::Reply(None) => Err("no kernel error reported!"),
            MessageKind::Request(_) => Err("requests are not replies :squint:"),
            MessageKind::ApplicationSpecific(_) => Err("application specifics are not replies :squint:"),
        }
    }
}

impl TryFrom<Message> for Option<KError> {
    type Error = &'static str;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        match msg.kind {
            MessageKind::Reply(Some(_)) => Ok(Some(KError::try_from(msg)?)),
            MessageKind::Reply(None) => Ok(None),
            MessageKind::Request(_) => Err("requests are not replies :squint:"),
            MessageKind::ApplicationSpecific(_) => Err("application specifics are not replies :squint:"),
        }
    }
}

#[derive(Debug)]
#[repr(C, usize)]
pub enum AccessError {
    Read(*const u8),
    Write(*mut u8),
}
