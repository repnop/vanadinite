// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    io::{ConsoleDevice},
    mem::{paging::VirtualAddress, user::RawUserSlice},
    task::Task,
};
use librust::{
    error::{SyscallError},
};

pub fn print(task: &mut Task, start: VirtualAddress, len: usize)-> Result<(), SyscallError> {
    let user_slice = RawUserSlice::readable(start, len);
    let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
        Ok(slice) => slice,
        Err((addr, e)) => {
            log::error!("Bad memory from process: {:?}", e);
            return Err(SyscallError::InvalidArgument(0));
        }
    };

    log::trace!("Attempting to print memory at {:#p} (len={})", start, len);

    let mut console = crate::io::CONSOLE.lock();
    user_slice.with(|bytes| bytes.iter().copied().for_each(|b| console.write(b)));

    Ok(())
}
