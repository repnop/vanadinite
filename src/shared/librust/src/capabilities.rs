// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CapabilityPtr(usize);

impl CapabilityPtr {
    pub const fn from_raw(n: usize) -> Self {
        Self(n)
    }

    pub const fn from_raw_parts(id: CapabilityId, kind: CapabilityType) -> Self {
        Self((id.0 << 4) | kind.to_usize())
    }

    pub const fn value(self) -> usize {
        self.0
    }

    pub const fn id(self) -> CapabilityId {
        CapabilityId::from_raw(self.0 >> 4)
    }

    pub const fn kind(self) -> CapabilityType {
        CapabilityType::from_raw(self.0 & 0b1111)
    }

    pub const fn into_raw_parts(self) -> (CapabilityId, CapabilityType) {
        (CapabilityId::from_raw(self.0 >> 4), CapabilityType::from_raw(self.0 & 0b1111))
    }

    pub fn get_memory_region(self) -> Option<*mut [u8]> {
        match self.kind() {
            CapabilityType::Bundle(size) | CapabilityType::Memory(size) => {
                Some(core::ptr::from_raw_parts_mut(core::ptr::from_exposed_addr_mut(self.id().0 << 4), size.size().0))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CapabilityId(usize);

impl CapabilityId {
    pub const fn from_raw(id: usize) -> Self {
        Self(id)
    }

    pub const fn get(self) -> usize {
        self.0
    }

    pub fn from_ptr<T: ?Sized>(ptr: *mut T) -> Self {
        assert!(ptr.addr() & 0b1111 == 0);

        Self(ptr.addr() >> 4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityType {
    Bundle(MemorySize),
    Endpoint,
    Memory(MemorySize),
    Mmio,
    ReplyEndpoint,
}

impl CapabilityType {
    pub const fn to_usize(self) -> usize {
        match self {
            CapabilityType::Bundle(size) => 0b1100 | (size as usize),
            CapabilityType::Endpoint => 0b0000,
            CapabilityType::Memory(size) => 0b1000 | (size as usize),
            CapabilityType::Mmio => 0b0001,
            CapabilityType::ReplyEndpoint => 0b0010,
        }
    }

    pub const fn from_raw(value: usize) -> Self {
        match value {
            0b0000 => Self::Endpoint,
            0b0001 => Self::Mmio,
            0b0010 => Self::ReplyEndpoint,
            n => match (n & 0b1100) >> 2 {
                0b10 => match n & 0b11 {
                    const { MemorySize::Tiny as usize } => Self::Memory(MemorySize::Tiny),
                    const { MemorySize::Small as usize } => Self::Memory(MemorySize::Small),
                    const { MemorySize::Medium as usize } => Self::Memory(MemorySize::Medium),
                    const { MemorySize::Large as usize } => Self::Memory(MemorySize::Large),
                    _ => unreachable!(),
                },
                0b11 => match n & 0b11 {
                    const { MemorySize::Tiny as usize } => Self::Bundle(MemorySize::Tiny),
                    const { MemorySize::Small as usize } => Self::Bundle(MemorySize::Small),
                    const { MemorySize::Medium as usize } => Self::Bundle(MemorySize::Medium),
                    const { MemorySize::Large as usize } => Self::Bundle(MemorySize::Large),
                    _ => unreachable!(),
                },
                _ => panic!("invalid `CapabilityPtr` value"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum MemorySize {
    /// 4 KiB
    Tiny = 0b00,
    /// 64 KiB
    Small = 0b01,
    /// 512 KiB
    Medium = 0b10,
    /// 2 MiB
    Large = 0b11,
}

impl MemorySize {
    pub const fn size(self) -> crate::units::Bytes {
        crate::units::Bytes(match self {
            MemorySize::Large => 4 * 1024,
            MemorySize::Medium => 64 * 1024,
            MemorySize::Small => 512 * 1024,
            MemorySize::Tiny => 2 * 1024 * 1024,
        })
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
