// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::syscalls::mem::MemoryPermissions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CapabilityPtr(usize);

impl CapabilityPtr {
    pub const fn new(n: usize) -> Self {
        Self(n)
    }

    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CapabilityRights(usize);

impl CapabilityRights {
    pub const NONE: Self = Self(0);
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXECUTE: Self = Self(1 << 2);
    pub const GRANT: Self = Self(1 << 3);
    pub const MOVE: Self = Self(1 << 4);
}

impl CapabilityRights {
    pub fn new(value: usize) -> Self {
        Self(value & 0x1F)
    }

    pub fn is_superset(self, other: Self) -> bool {
        // `MOVE` rights are sticky and so must be set in both or neither
        (self.0 | !other.0) == usize::MAX && ((self & Self::MOVE) == (other & Self::MOVE))
    }

    pub fn value(self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for CapabilityRights {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        CapabilityRights(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for CapabilityRights {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = CapabilityRights(self.0 | rhs.0);
    }
}

impl core::ops::BitAnd for CapabilityRights {
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

impl core::fmt::Debug for CapabilityRights {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CapabilityRights(")?;

        match *self {
            CapabilityRights::NONE => write!(f, "NONE")?,
            rights => {
                let mut tracking = [None, None, None, None];

                if rights & CapabilityRights::READ {
                    tracking[0] = Some("READ");
                }

                if rights & CapabilityRights::WRITE {
                    tracking[1] = Some("WRITE");
                }

                if rights & CapabilityRights::EXECUTE {
                    tracking[2] = Some("EXECUTE");
                }

                if rights & CapabilityRights::GRANT {
                    tracking[3] = Some("GRANT");
                }

                let (last_idx, _) = tracking.iter().flatten().enumerate().last().unwrap();
                for (i, right) in tracking.into_iter().flatten().enumerate() {
                    write!(f, "{}", right)?;

                    if i != last_idx {
                        write!(f, "| ")?;
                    }
                }
            }
        }

        write!(f, ")")
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Capability {
    pub cptr: CapabilityPtr,
    pub rights: CapabilityRights,
}

impl Capability {
    pub fn new(cptr: CapabilityPtr, rights: CapabilityRights) -> Self {
        Self { cptr, rights }
    }
}

impl Default for Capability {
    fn default() -> Self {
        Self { cptr: CapabilityPtr(usize::MAX), rights: CapabilityRights::NONE }
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CapabilityWithDescription {
    pub capability: Capability,
    pub description: CapabilityDescription,
}

// FIXME: perhaps use a safer representation? hmm
#[derive(Debug, Clone, Copy)]
#[repr(C, usize)]
pub enum CapabilityDescription {
    Channel = 0,
    Memory { ptr: *mut u8, len: usize, permissions: MemoryPermissions } = 1,
    MappedMmio { ptr: *mut u8, len: usize, n_interrupts: usize } = 2,
    Bundle { ptr: *mut u8, len: usize } = 3,
}

impl Default for CapabilityDescription {
    fn default() -> Self {
        Self::Channel
    }
}

unsafe impl Send for CapabilityDescription {}
unsafe impl Sync for CapabilityDescription {}
