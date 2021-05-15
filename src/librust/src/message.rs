// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    error::{self, AccessError, KError},
    task::Tid,
};
use core::{convert::TryInto, num::NonZeroUsize};

#[derive(Debug)]
#[repr(C)]
pub struct Message {
    /// `t1`
    pub sender: Sender,
    /// Descriminant `t2`, value `t3`
    pub kind: MessageKind,
    /// `t4`
    pub fid: usize,
    /// `a0`-`a7`
    pub arguments: [usize; 8],
}

impl From<KError> for Message {
    fn from(kerror: KError) -> Self {
        match kerror {
            KError::InvalidRecipient => Self {
                // TODO: is this ok?
                sender: Sender::kernel(),
                kind: MessageKind::Reply(Some(error::INVALID_RECIPIENT.try_into().unwrap())),
                fid: 0,
                arguments: [0; 8],
            },
            KError::InvalidMessage => Self {
                sender: Sender::kernel(),
                kind: MessageKind::Reply(Some(error::INVALID_MESSAGE.try_into().unwrap())),
                fid: 0,
                arguments: [0; 8],
            },
            KError::InvalidAccess(access_error) => match access_error {
                AccessError::Read(ptr) => Self {
                    sender: Sender::kernel(),
                    kind: MessageKind::Reply(Some(error::INVALID_ACCESS.try_into().unwrap())),
                    fid: 0,
                    arguments: [0, ptr as usize, 0, 0, 0, 0, 0, 0],
                },
                AccessError::Write(ptr) => Self {
                    sender: Sender::kernel(),
                    kind: MessageKind::Reply(Some(error::INVALID_ACCESS.try_into().unwrap())),
                    fid: 0,
                    arguments: [1, ptr as usize, 0, 0, 0, 0, 0, 0],
                },
            },
            KError::InvalidSyscall(id) => Self {
                sender: Sender::kernel(),
                kind: MessageKind::Reply(Some(error::INVALID_SYSCALL.try_into().unwrap())),
                fid: 0,
                arguments: [id, 0, 0, 0, 0, 0, 0, 0],
            },
            KError::InvalidArgument(idx) => Self {
                sender: Sender::kernel(),
                kind: MessageKind::Reply(Some(error::INVALID_ARGUMENT.try_into().unwrap())),
                fid: 0,
                arguments: [idx, 0, 0, 0, 0, 0, 0, 0],
            },
        }
    }
}

impl From<Option<KError>> for Message {
    fn from(kerror: Option<KError>) -> Self {
        match kerror {
            Some(kerror) => Self::from(kerror),
            None => Self { sender: Sender::kernel(), kind: MessageKind::Reply(None), fid: 0, arguments: [0; 8] },
        }
    }
}

#[derive(Debug)]
#[repr(C, usize)]
#[non_exhaustive]
pub enum MessageKind {
    ApplicationSpecific(usize),
    Request(Option<NonZeroUsize>),
    Reply(Option<NonZeroUsize>),
    Notification(usize),
}

impl MessageKind {
    pub fn from_parts(descriminant: usize, value: usize) -> Option<Self> {
        match descriminant {
            0 => Some(MessageKind::ApplicationSpecific(value)),
            1 => Some(MessageKind::Request(match value {
                0 => None,
                _ => Some(NonZeroUsize::new(value).unwrap()),
            })),
            2 => Some(MessageKind::Reply(match value {
                0 => None,
                _ => Some(NonZeroUsize::new(value).unwrap()),
            })),
            3 => Some(MessageKind::Notification(value)),
            _ => None,
        }
    }

    pub fn into_parts(self) -> (usize, usize) {
        match self {
            MessageKind::ApplicationSpecific(value) => (0, value),
            MessageKind::Request(value) => (1, value.map(|v| v.get()).unwrap_or(0)),
            MessageKind::Reply(value) => (2, value.map(|v| v.get()).unwrap_or(0)),
            MessageKind::Notification(value) => (3, value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Sender(usize);

impl Sender {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn dummy() -> Self {
        Sender(usize::max_value())
    }

    pub const fn kernel() -> Self {
        Sender(0)
    }

    pub fn task(tid: Tid) -> Self {
        Sender(tid.value())
    }

    pub fn value(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Recipient(usize);

impl Recipient {
    pub const fn new(value: usize) -> Self {
        Recipient(value)
    }

    pub const fn kernel() -> Self {
        Recipient(0)
    }

    pub fn task(tid: Tid) -> Self {
        Recipient(tid.value())
    }

    pub fn value(self) -> usize {
        self.0
    }

    pub fn is_kernel(self) -> bool {
        self.0 == 0
    }

    pub fn is_task(self) -> bool {
        !self.is_kernel()
    }
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub enum Sender {
//     Kernel,
//     Task(Tid),
// }

// impl Sender {
//     pub fn into_raw(self) -> raw::Sender {
//         match self {
//             Sender::Kernel => raw::Sender::kernel(),
//             Self::Task(tid) => raw::Sender::task(tid),
//         }
//     }
// }

// impl From<raw::Sender> for Sender {
//     fn from(s: raw::Sender) -> Self {
//         match s.0 {
//             0 => Sender::Kernel,
//             tid => Sender::Task(Tid::new(NonZeroUsize::new(tid).unwrap())),
//         }
//     }
// }
