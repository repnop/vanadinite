// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::SyscallOutcome;
use crate::{
    io::{ConsoleDevice, INPUT_QUEUE},
    mem::{paging::VirtualAddress, user::RawUserSlice},
    task::Task,
};
use librust::{
    error::{AccessError, KError},
    message::Message,
};

pub fn print(task: &mut Task, start: VirtualAddress, len: usize) -> SyscallOutcome {
    let user_slice = RawUserSlice::readable(start, len);
    let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
        Ok(slice) => slice,
        Err((addr, e)) => {
            log::error!("Bad memory from process: {:?}", e);
            return SyscallOutcome::Err(KError::InvalidAccess(AccessError::Read(addr.as_ptr())));
        }
    };

    log::trace!("Attempting to print memory at {:#p} (len={})", start, len);

    let mut console = crate::io::CONSOLE.lock();
    user_slice.with(|bytes| bytes.iter().copied().for_each(|b| console.write(b)));

    SyscallOutcome::Processed(Message::default())
}

pub fn read_stdin(task: &mut Task, start: VirtualAddress, len: usize) -> SyscallOutcome {
    let user_slice = RawUserSlice::writable(start, len);
    let mut user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
        Ok(slice) => slice,
        Err((addr, e)) => {
            log::error!("Bad memory from process: {:?}", e);
            return SyscallOutcome::Err(KError::InvalidAccess(AccessError::Write(addr.as_mut_ptr())));
        }
    };

    log::trace!("Attempting to write to memory at {:#p} (len={})", start, len);

    let mut n_written = 0;
    user_slice.with(|bytes| {
        for byte in bytes {
            let value = match INPUT_QUEUE.pop() {
                Some(v) => v,
                None => break,
            };
            *byte = value;
            n_written += 1;
        }
    });

    SyscallOutcome::Processed(Message::from(n_written))
}
