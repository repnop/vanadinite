// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    error::{self, AccessError, KError},
    syscalls::{channel::ChannelId, Syscall},
    task::Tid,
};
use core::{convert::TryInto, num::NonZeroUsize};

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct Message {
    pub contents: [usize; 13],
}

#[derive(Debug)]
#[repr(C)]
pub struct SyscallRequest {
    pub syscall: Syscall,
    pub arguments: [usize; 12],
}

impl From<SyscallRequest> for Message {
    fn from(req: SyscallRequest) -> Self {
        let mut contents = [0; 13];
        contents[1..].copy_from_slice(&req.arguments);
        contents[0] = req.syscall as usize;

        Self { contents }
    }
}

#[derive(Debug, Clone, Copy)]
#[must_use]
pub enum SyscallResult<T, E = KError> {
    Ok(T),
    Err(E),
}

impl<T, E> core::ops::Try for SyscallResult<T, E> {
    type Output = T;
    type Residual = SyscallResult<!, E>;

    fn from_output(output: Self::Output) -> Self {
        Self::Ok(output)
    }

    fn branch(self) -> core::ops::ControlFlow<Self::Residual, Self::Output> {
        match self {
            Self::Ok(t) => core::ops::ControlFlow::Continue(t),
            Self::Err(e) => core::ops::ControlFlow::Break(SyscallResult::Err(e)),
        }
    }
}

impl<T, E, F: From<E>> core::ops::FromResidual<SyscallResult<!, E>> for SyscallResult<T, F> {
    fn from_residual(residual: SyscallResult<!, E>) -> Self {
        match residual {
            SyscallResult::Ok(_) => unreachable!(),
            SyscallResult::Err(e) => Self::Err(From::from(e)),
        }
    }
}

impl<T, E: core::fmt::Debug> SyscallResult<T, E> {
    #[track_caller]
    pub fn unwrap(self) -> T {
        match self {
            SyscallResult::Ok(t) => t,
            SyscallResult::Err(e) => panic!("unwrapped a syscall error: {:?}", e),
        }
    }
}

impl<T, E> SyscallResult<T, E> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> SyscallResult<U, E> {
        match self {
            Self::Ok(t) => SyscallResult::Ok(f(t)),
            Self::Err(e) => SyscallResult::Err(e),
        }
    }

    pub fn map_err<U>(self, f: impl FnOnce(E) -> U) -> SyscallResult<T, U> {
        match self {
            Self::Ok(t) => SyscallResult::Ok(t),
            Self::Err(e) => SyscallResult::Err(f(e)),
        }
    }
}

impl From<KError> for Message {
    fn from(kerror: KError) -> Self {
        match kerror {
            KError::InvalidRecipient => {
                Self { contents: [error::INVALID_RECIPIENT, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }
            }
            KError::InvalidMessage => Self { contents: [error::INVALID_MESSAGE, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
            KError::InvalidAccess(access_error) => match access_error {
                AccessError::Read(ptr) => Self {
                    contents: [
                        error::INVALID_ACCESS,
                        error::ACCESS_ERROR_READ,
                        ptr as usize,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ],
                },
                AccessError::Write(ptr) => Self {
                    contents: [
                        error::INVALID_ACCESS,
                        error::ACCESS_ERROR_WRITE,
                        ptr as usize,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ],
                },
            },
            KError::InvalidSyscall(id) => {
                Self { contents: [error::INVALID_SYSCALL, id, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }
            }
            KError::InvalidArgument(idx) => {
                Self { contents: [error::INVALID_ARGUMENT, idx, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }
            }
            KError::NoMessages => Self { contents: [error::NO_MESSAGES, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
        }
    }
}

impl<T: Into<Message>, E: Into<Message>> From<SyscallResult<T, E>> for Message {
    fn from(kres: SyscallResult<T, E>) -> Self {
        match kres {
            SyscallResult::Ok(val) => val.into(),
            SyscallResult::Err(e) => e.into(),
        }
    }
}

impl<T: From<Message>, E: From<Message>> From<(bool, Message)> for SyscallResult<T, E> {
    fn from((err, msg): (bool, Message)) -> Self {
        match err {
            false => SyscallResult::Ok(T::from(msg)),
            true => SyscallResult::Err(E::from(msg)),
        }
    }
}

impl From<Message> for () {
    fn from(_: Message) -> Self {}
}

impl From<()> for Message {
    fn from(_: ()) -> Self {
        Self { contents: [0; 13] }
    }
}

impl From<usize> for Message {
    fn from(t: usize) -> Self {
        let mut contents = [0; 13];
        contents[0] = t;

        Self { contents }
    }
}

impl From<Message> for usize {
    fn from(msg: Message) -> Self {
        msg.contents[0]
    }
}

impl<T> From<Message> for *mut T {
    fn from(msg: Message) -> Self {
        msg.contents[0] as *mut T
    }
}

impl<T> From<Message> for *const T {
    fn from(msg: Message) -> Self {
        msg.contents[0] as *const T
    }
}

macro_rules! impl_trait_for_tuples {
    (
        $(
            $ty:ty
            =>
            (
                $($t:tt),*
            )
        ),+
    ) => {
        $(
            impl From<$ty> for Message {
                fn from(t: $ty) -> Self {
                    let mut contents = [0; 13];

                    $(
                        contents[$t] = t.$t as _;
                    )*

                    Self { contents }
                }
            }

            impl From<Message> for $ty {
                fn from(msg: Message) -> $ty {
                    ($(msg.contents[$t] as _),*,)
                }
            }
        )+
    };
}

impl_trait_for_tuples! {
    (usize,) => (0),
    (usize, usize) => (0, 1),
    (usize, usize, usize) => (0, 1, 2),
    (usize, usize, usize, usize) => (0, 1, 2, 3),
    (usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4),
    (usize, usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4, 5),
    (usize, usize, usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4, 5, 6),
    (usize, usize, usize, usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4, 5, 6, 7),
    (usize, usize, usize, usize, usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4, 5, 6, 7, 8),
    (usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9),
    (usize, usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10),
    (*mut u8,) => (0),
    (*mut u8, *mut u8) => (0, 1),
    (*mut u8, *mut u8, *mut u8) => (0, 1, 2),
    (*mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4, 5),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4, 5, 6),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4, 5, 6, 7),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4, 5, 6, 7, 8),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9),
    (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)
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
        Sender(usize::MAX)
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

    pub fn is_kernel(self) -> bool {
        self.0 == 0
    }

    pub fn is_task(self) -> bool {
        !self.is_kernel()
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

#[derive(Debug, Clone, Copy)]
#[repr(C, usize)]
pub enum KernelNotification {
    ChannelRequest(Tid),
    ChannelOpened(ChannelId),
    ChannelRequestDenied,
    InterruptOccurred(usize),
    NewChannelMessage(ChannelId),
}

pub const NOTIFICATION_CHANNEL_REQUEST: usize = 0;
pub const NOTIFICATION_CHANNEL_OPENED: usize = 1;
pub const NOTIFICATION_CHANNEL_REQUEST_DENIED: usize = 2;
pub const NOTIFICATION_INTERRUPT_OCCURRED: usize = 3;
pub const NOTIFICATION_NEW_CHANNEL_MESSAGE: usize = 4;

impl From<Message> for KernelNotification {
    fn from(message: Message) -> Self {
        match message.contents[0] {
            NOTIFICATION_CHANNEL_REQUEST => {
                KernelNotification::ChannelRequest(Tid::new(message.contents[1].try_into().unwrap()))
            }
            NOTIFICATION_CHANNEL_OPENED => KernelNotification::ChannelOpened(ChannelId::new(message.contents[1])),
            NOTIFICATION_CHANNEL_REQUEST_DENIED => KernelNotification::ChannelRequestDenied,
            NOTIFICATION_INTERRUPT_OCCURRED => KernelNotification::InterruptOccurred(message.contents[1]),
            NOTIFICATION_NEW_CHANNEL_MESSAGE => {
                KernelNotification::NewChannelMessage(ChannelId::new(message.contents[1]))
            }
            _ => unreachable!("bad KernelNotification or used this impl one something that wasn't "),
        }
    }
}

impl From<KernelNotification> for Message {
    fn from(notif: KernelNotification) -> Self {
        let mut contents = [0; 13];

        match notif {
            KernelNotification::ChannelRequest(tid) => {
                contents[0] = NOTIFICATION_CHANNEL_REQUEST;
                contents[1] = tid.value();
            }
            KernelNotification::ChannelOpened(id) => {
                contents[0] = NOTIFICATION_CHANNEL_OPENED;
                contents[1] = id.value();
            }
            KernelNotification::ChannelRequestDenied => {
                contents[0] = NOTIFICATION_CHANNEL_REQUEST_DENIED;
            }
            KernelNotification::InterruptOccurred(n) => {
                contents[0] = NOTIFICATION_INTERRUPT_OCCURRED;
                contents[1] = n;
            }
            KernelNotification::NewChannelMessage(id) => {
                contents[0] = NOTIFICATION_NEW_CHANNEL_MESSAGE;
                contents[1] = id.value();
            }
        }

        Self { contents }
    }
}
