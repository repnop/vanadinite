// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::{
        manager::AddressRegionKind,
        paging::{PhysicalAddress, VirtualAddress},
        region::SharedPhysicalRegion,
        PageRange,
    },
    syscall::channel::{ChannelEndpoint, ReplyEndpoint},
};
use alloc::collections::BTreeMap;
use core::ops::Range;
use librust::capabilities::{CapabilityPtr, CapabilityRights};

#[derive(Debug, Clone, Copy)]
pub struct Occupied;

pub struct CapabilitySpace {
    inner: BTreeMap<CapabilityPtr, Capability>,
}

impl CapabilitySpace {
    pub fn new() -> Self {
        Self { inner: BTreeMap::new() }
    }

    // FIXME: is there a better method to use here? maybe split out special
    // caps? unsure
    /// Mint a new capability with the given [`CapabilityPtr`] value. Returns
    /// `Err(())` if the [`CapabilityPtr`] value already exists.
    pub fn mint_with_id(&mut self, cptr: CapabilityPtr, capability: Capability) -> Result<(), Occupied> {
        match self.inner.get(&cptr).is_some() {
            true => Err(Occupied),
            false => {
                self.inner.insert(cptr, capability);
                Ok(())
            }
        }
    }

    pub fn mint_with(&mut self, f: impl FnOnce(CapabilityPtr) -> Capability) -> CapabilityPtr {
        let cptr = CapabilityPtr::new(self.inner.keys().max().map(|c| c.value() + 1).unwrap_or(0));
        self.inner.insert(cptr, f(cptr));
        cptr
    }

    /// Create a new [`CapabilityPtr`] representing the given [`Capability`]
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

    pub fn remove(&mut self, cptr: CapabilityPtr) -> Option<Capability> {
        self.inner.remove(&cptr)
    }

    pub fn resolve_mut(&mut self, cptr: CapabilityPtr) -> Option<&mut Capability> {
        self.inner.get_mut(&cptr)
    }

    pub fn all(&self) -> impl Iterator<Item = (&CapabilityPtr, &Capability)> {
        self.inner.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Capability {
    pub resource: CapabilityResource,
    pub rights: CapabilityRights,
}

#[derive(Debug, Clone)]
pub enum CapabilityResource {
    Bundle(CapabilityBundle),
    Channel(ChannelEndpoint),
    Mmio(MmioRegion),
    Reply(ReplyEndpoint),
    SharedMemory(SharedMemory),
}

#[derive(Debug, Clone)]
pub struct SharedMemory {
    pub physical_region: SharedPhysicalRegion,
    pub virtual_range: PageRange<VirtualAddress>,
    pub kind: AddressRegionKind,
}

#[derive(Debug, Clone)]
pub struct MmioRegion {
    pub physical_range: PageRange<PhysicalAddress>,
    pub virtual_range: PageRange<VirtualAddress>,
    pub interrupts: alloc::vec::Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct CapabilityBundle {
    pub endpoint: ChannelEndpoint,
    pub shared_memory: SharedMemory,
}
