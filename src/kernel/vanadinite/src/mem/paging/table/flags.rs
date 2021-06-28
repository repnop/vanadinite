// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Clone, Copy, PartialEq)]
pub struct Flags(u8);

impl Flags {
    pub const fn new(n: u8) -> Self {
        Self(n)
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    pub fn matchable(self) -> FlagsStruct {
        FlagsStruct {
            valid: self & VALID,
            read: self & READ,
            write: self & WRITE,
            execute: self & EXECUTE,
            user: self & USER,
            global: self & GLOBAL,
            accessed: self & ACCESSED,
            dirty: self & DIRTY,
        }
    }
}

impl core::fmt::Debug for Flags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", if *self & DIRTY { "d" } else { "-" })?;
        write!(f, "{}", if *self & ACCESSED { "a" } else { "-" })?;
        write!(f, "{}", if *self & GLOBAL { "g" } else { "-" })?;
        write!(f, "{}", if *self & USER { "u" } else { "-" })?;
        write!(f, "{}", if *self & EXECUTE { "x" } else { "-" })?;
        write!(f, "{}", if *self & WRITE { "w" } else { "-" })?;
        write!(f, "{}", if *self & READ { "r" } else { "-" })?;
        write!(f, "{}", if *self & VALID { "v" } else { "-" })?;

        Ok(())
    }
}

pub const VALID: Flags = Flags(0b0000_0001);
pub const READ: Flags = Flags(0b0000_0010);
pub const WRITE: Flags = Flags(0b0000_0100);
pub const EXECUTE: Flags = Flags(0b0000_1000);
pub const USER: Flags = Flags(0b0001_0000);
pub const GLOBAL: Flags = Flags(0b0010_0000);
pub const ACCESSED: Flags = Flags(0b0100_0000);
pub const DIRTY: Flags = Flags(0b1000_0000);

impl core::ops::BitOr for Flags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Flags(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for Flags {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = Flags(self.0 | rhs.0);
    }
}

impl core::ops::BitAnd for Flags {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

pub struct FlagsStruct {
    pub valid: bool,
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub user: bool,
    pub global: bool,
    pub accessed: bool,
    pub dirty: bool,
}
