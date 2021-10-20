// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alloc::collections::BTreeMap;
use librust::{capabilities::CapabilityPtr, syscalls::channel::ChannelId};

pub struct CapabilitySpace {
    inner: BTreeMap<CapabilityPtr, Capability>,
}

impl CapabilitySpace {
    pub fn new() -> Self {
        Self { inner: BTreeMap::new() }
    }

    pub fn mint(&mut self, capability: Capability) -> CapabilityPtr {
        // FIXME: Uncomment & improve
        // let time = crate::csr::time::read() as usize;
        let cptr = CapabilityPtr::new(self.inner.keys().max().map(|c| c.value()).unwrap_or(0));

        // This should go away when there's a better RNG method or whathaveyou
        assert!(self.inner.insert(cptr, capability).is_none());

        cptr
    }

    pub fn resolve(&self, cptr: CapabilityPtr) -> Option<&Capability> {
        self.inner.get(&cptr)
    }

    pub fn resolve_mut(&mut self, cptr: CapabilityPtr) -> Option<&mut Capability> {
        self.inner.get_mut(&cptr)
    }

    pub fn all(&self) -> impl Iterator<Item = (&CapabilityPtr, &Capability)> {
        self.inner.iter()
    }
}

pub struct Capability {
    pub resource: CapabilityResource,
    pub rights: CapabilityRights,
}

pub enum CapabilityResource {
    Channel(ChannelId),
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct CapabilityRights(u8);

impl CapabilityRights {
    pub const READ: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const EXECUTE: Self = Self(4);
    pub const GRANT: Self = Self(8);
}

impl CapabilityRights {
    pub fn is_superset(self, other: Self) -> bool {
        (self.0 | !other.0) == u8::MAX
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
