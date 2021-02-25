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

pub const READ: Permissions = Permissions(0b00001);
pub const WRITE: Permissions = Permissions(0b00010);
pub const EXECUTE: Permissions = Permissions(0b00100);
pub const USER: Permissions = Permissions(0b01000);
pub const GLOBAL: Permissions = Permissions(0b10000);

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
