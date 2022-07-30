// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::sync::SyncRefCell;
use librust::{
    capabilities::{Capability, CapabilityRights},
    mem::MemoryAllocation,
    syscalls::channel::ChannelMessage,
    units::Bytes,
};

pub(crate) struct StdoutInner(SyncRefCell<Option<(usize, MemoryAllocation)>>);

impl StdoutInner {
    pub const fn new() -> Self {
        Self(SyncRefCell::new(None))
    }
}

static STDOUT: StdoutInner = StdoutInner::new();

pub struct Stdout;
impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // let stdio = match crate::env::lookup_capability("stdio") {
        //     Some(stdio) => crate::ipc::IpcChannel::new(stdio.capability.cptr),
        //     None => return Ok(()),
        // };

        // let mut inner = STDOUT.0.borrow_mut();

        // if inner.is_none() {
        //     *inner = Some((0, MemoryAllocation::public_rw(Bytes(4096)).expect("failed to allocate memory for stdout")));
        // }

        // let (position, mem) = inner.as_mut().unwrap();
        // let cptr = mem.cptr;
        // // SAFETY: we don't ever copy the pointer out
        // let buffer = unsafe { mem.as_mut() };
        // for byte in s.bytes() {
        //     buffer[*position] = byte;

        //     if byte == b'\n' || *position == buffer.len() - 1 {
        //         let msg = ChannelMessage([1, *position, 0, 0, 0, 0, 0]);
        //         let _ = stdio.send(msg, &[Capability { cptr, rights: CapabilityRights::READ }]);
        //     } else {
        //         *position += 1;
        //     }
        // }

        let _ = librust::syscalls::io::debug_print(s.as_bytes());
        Ok(())
    }
}
