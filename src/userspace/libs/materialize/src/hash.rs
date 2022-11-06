// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

const K: u64 = 0x517cc1b727220a95;

pub struct FxHasher(u64);

impl FxHasher {
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn hash(self, value: u64) -> Self {
        Self((self.0.rotate_left(5) ^ value).wrapping_mul(K))
    }

    pub const fn finish(self) -> u64 {
        self.0
    }
}
