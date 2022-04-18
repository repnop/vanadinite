// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;
use crate::{task::Tid, syscalls::Syscall};

#[inline(always)]
pub fn exit() -> ! {
    unsafe { super::syscall0r0(Syscall::Exit) };
    unreachable!()
}

#[inline]
pub fn current_tid() -> Tid {
    Tid::new(
        NonZeroUsize::new(
            unsafe { super::syscall0r1(Syscall::GetTid).unwrap() }
        )
        .unwrap(),
    )
}