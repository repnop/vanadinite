// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ops::{BitAnd, BitOr};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Permissions(u8);

impl Permissions {
    pub fn valid(self) -> bool {
        !matches!(self.0 & 0b111, 0b010 | 0b110)
    }

    pub fn as_bits(self) -> usize {
        self.0 as usize
    }
}

// FIXME: wtf why aren't these full values and why no valid
pub const READ: Permissions = Permissions(0b0000_0001);
pub const WRITE: Permissions = Permissions(0b0000_0010);
pub const EXECUTE: Permissions = Permissions(0b0000_0100);
pub const USER: Permissions = Permissions(0b0000_1000);
pub const GLOBAL: Permissions = Permissions(0b0001_0000);
pub const DIRTY: Permissions = Permissions(0b0100_0000);
pub const ACCESSED: Permissions = Permissions(0b0010_0000);

impl BitOr for Permissions {
    type Output = Permissions;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitAnd for Permissions {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}
