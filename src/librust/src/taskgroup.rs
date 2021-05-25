// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub unsafe trait TaskGroupShareable {}

#[cfg(feature = "alloc")]
unsafe impl<T: TaskGroupShareable, A: TaskGroupShareable + alloc::alloc::Allocator> TaskGroupShareable
    for alloc::boxed::Box<T, A>
{
}

#[cfg(feature = "alloc")]
unsafe impl<T: TaskGroupShareable, A: TaskGroupShareable + alloc::alloc::Allocator> TaskGroupShareable
    for alloc::vec::Vec<T, A>
{
}

macro_rules! implTaskGroupShareable {
    ($($t:ty),+) => {
        $(
            unsafe impl TaskGroupShareable for $t {}
        )+
    };
}

implTaskGroupShareable!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize, char);
