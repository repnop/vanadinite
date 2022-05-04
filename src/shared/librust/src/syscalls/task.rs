// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{syscalls::Syscall, task::Tid};
use core::num::NonZeroUsize;

#[inline(always)]
pub fn exit() -> ! {
    unsafe { crate::syscall!(Syscall::Exit, 1) };
    unreachable!()
}

#[inline]
pub fn current_tid() -> Tid {
    Tid::new(NonZeroUsize::new(unsafe { super::syscall0r1(Syscall::GetTid).unwrap() }).unwrap())
}
