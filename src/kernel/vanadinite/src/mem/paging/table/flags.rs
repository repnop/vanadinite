// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Clone, Copy, PartialEq)]
pub struct Flags(u8);

impl Flags {
    pub const VALID: Flags = Flags(0b0000_0001);
    pub const READ: Flags = Flags(0b0000_0010);
    pub const WRITE: Flags = Flags(0b0000_0100);
    pub const EXECUTE: Flags = Flags(0b0000_1000);
    pub const USER: Flags = Flags(0b0001_0000);
    pub const GLOBAL: Flags = Flags(0b0010_0000);
    pub const ACCESSED: Flags = Flags(0b0100_0000);
    pub const DIRTY: Flags = Flags(0b1000_0000);

    pub const fn new(n: u8) -> Self {
        Self(n)
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    pub fn matchable(self) -> FlagsStruct {
        FlagsStruct {
            valid: self & Self::VALID,
            read: self & Self::READ,
            write: self & Self::WRITE,
            execute: self & Self::EXECUTE,
            user: self & Self::USER,
            global: self & Self::GLOBAL,
            accessed: self & Self::ACCESSED,
            dirty: self & Self::DIRTY,
        }
    }
}

impl core::fmt::Debug for Flags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", if *self & Self::DIRTY { "d" } else { "-" })?;
        write!(f, "{}", if *self & Self::ACCESSED { "a" } else { "-" })?;
        write!(f, "{}", if *self & Self::GLOBAL { "g" } else { "-" })?;
        write!(f, "{}", if *self & Self::USER { "u" } else { "-" })?;
        write!(f, "{}", if *self & Self::EXECUTE { "x" } else { "-" })?;
        write!(f, "{}", if *self & Self::WRITE { "w" } else { "-" })?;
        write!(f, "{}", if *self & Self::READ { "r" } else { "-" })?;
        write!(f, "{}", if *self & Self::VALID { "v" } else { "-" })?;

        Ok(())
    }
}

impl core::ops::BitOr for Flags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for Flags {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = Self(self.0 | rhs.0);
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
