// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tid(usize);

impl Tid {
    pub fn new(tid: NonZeroUsize) -> Self {
        Self(tid.get())
    }

    pub fn value(self) -> usize {
        self.0
    }

    pub fn current() -> Self {
        todo!("get tid")
    }
}
