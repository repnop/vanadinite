// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ops::Range;

use alloc::collections::BTreeMap;
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    syscalls::channel::ChannelId,
};

use crate::mem::{manager::AddressRegionKind, paging::VirtualAddress, region::SharedPhysicalRegion};

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
        let cptr = CapabilityPtr::new(self.inner.keys().max().map(|c| c.value() + 1).unwrap_or(0));

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

#[derive(Debug)]
pub enum CapabilityResource {
    Channel(ChannelId),
    Memory(SharedPhysicalRegion, Range<VirtualAddress>, AddressRegionKind),
}
