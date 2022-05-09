// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{syscalls::Syscall, task::Tid, error::RawSyscallError};
use core::num::NonZeroUsize;

#[inline(always)]
pub fn exit() -> ! {
    unsafe {
        core::arch::asm!("ecall", in("a0") Syscall::Exit as usize);
    }
    unreachable!()
}

#[inline]
pub fn current_tid() -> Tid {
    let error: usize;
    let tid: usize;

    unsafe { 
        core::arch::asm!(
            "ecall",
            inlateout("a0") Syscall::GetTid as usize => error,
            lateout("a1") tid,
        );
    }

    match RawSyscallError::optional(error) {
        Some(_) => unreachable!(),
        None => Tid::new(NonZeroUsize::new(tid).unwrap()),
    }
}
