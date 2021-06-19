// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::message::Message;

pub const INVALID_ACCESS: usize = 1;
pub const INVALID_MESSAGE: usize = 2;
pub const INVALID_RECIPIENT: usize = 3;
pub const INVALID_SYSCALL: usize = 4;
pub const INVALID_ARGUMENT: usize = 5;
pub const NO_MESSAGES: usize = 6;

pub const IS_KERROR: usize = 1;

#[derive(Debug)]
pub enum KError {
    InvalidAccess(AccessError),
    InvalidMessage,
    InvalidRecipient,
    InvalidSyscall(usize),
    InvalidArgument(usize),
    NoMessages,
}

impl From<Message> for KError {
    fn from(msg: Message) -> Self {
        match msg.contents[0] {
            const { INVALID_MESSAGE } => Self::InvalidMessage,
            const { INVALID_RECIPIENT } => Self::InvalidRecipient,
            const { INVALID_SYSCALL } => Self::InvalidSyscall(msg.contents[1]),
            const { INVALID_ARGUMENT } => Self::InvalidArgument(msg.contents[1]),
            const { INVALID_ACCESS } => Self::InvalidAccess(match msg.contents[1] {
                0 => AccessError::Read(msg.contents[2] as _),
                1 => AccessError::Write(msg.contents[2] as _),
                _ => unreachable!(),
            }),
            const { NO_MESSAGES } => Self::NoMessages,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
#[repr(C, usize)]
pub enum AccessError {
    Read(*const u8),
    Write(*mut u8),
}

pub const ACCESS_ERROR_READ: usize = 0;
pub const ACCESS_ERROR_WRITE: usize = 1;
