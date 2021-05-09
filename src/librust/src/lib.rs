// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(inline_const)]
#![no_std]
#![allow(incomplete_features)]

use core::convert::TryFrom;
use message::{Message, MessageKind};

pub mod capabilities;
pub mod error;
pub mod message;
pub mod syscalls;
pub mod task;

pub type KResult<T> = Result<T, error::KError>;

impl TryFrom<Message> for KResult<Message> {
    type Error = &'static str;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        match msg.kind {
            MessageKind::Request(_) | MessageKind::ApplicationSpecific(_) => Err("message not a reply"),
            MessageKind::Reply(err) => match err {
                Some(_) => Ok(Self::Err(error::KError::try_from(msg)?)),
                None => Ok(Self::Ok(msg)),
            },
        }
    }
}

impl TryFrom<Message> for () {
    type Error = &'static str;

    fn try_from(_: Message) -> Result<Self, Self::Error> {
        Ok(())
    }
}
