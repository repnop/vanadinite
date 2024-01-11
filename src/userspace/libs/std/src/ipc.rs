// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use librust::{
    error::SyscallError,
    syscalls::endpoint::{self, IpcMessage},
};

pub use librust::capabilities::{Capability, CapabilityPtr, CapabilityRights};
pub use librust::syscalls::endpoint::{ChannelReadFlags, EndpointMessage};

pub fn recv(flags: ChannelReadFlags) -> Result<IpcMessage, SyscallError> {
    loop {
        match endpoint::recv(flags)? {
            endpoint::Message::Ipc(res) => return Ok(res),
            endpoint::Message::Kernel(notif) => match notif {
                endpoint::KernelMessage::InterruptOccurred(id) => {
                    match &mut *crate::task::INTERRUPT_CALLBACK.borrow_mut() {
                        Some(callback) => callback(id),
                        None => {}
                    }
                }
            },
        }
    }
}
