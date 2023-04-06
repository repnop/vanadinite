// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::VirtualAddress;
use crate::mem::{paging::flags::Flags, region::MemoryRegion};
use alloc::collections::BTreeMap;
use core::ops::Range;

// TODO: probably could split this up slightly more and represent the
// {un}occupied regions as different types?
/// A region of memory allocated to a task
#[derive(Debug, PartialEq)]
pub struct AddressRegion {
    /// The underlying [`MemoryRegion`], which may or may not be backed by
    /// physical memory. `None` represents an unoccupied region.
    pub region: Option<MemoryRegion>,
    /// The region span
    pub span: Range<VirtualAddress>,
    /// The type of memory contained in the region, used for debugging purposes
    pub kind: AddressRegionKind,
    /// The permissions for the region
    pub permissions: Flags,
}

impl AddressRegion {
    pub fn is_unoccupied(&self) -> bool {
        self.region.is_none()
    }
}

/// Describes what type of memory the address region contains
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressRegionKind {
    Channel,
    Data,
    Guard,
    ReadOnly,
    Stack,
    Text,
    Tls,
    Unoccupied,
    UserAllocated,
    Dma,
    Mmio,
    UserSharedMemory,
}

#[derive(Debug)]
pub struct Userspace;
#[derive(Debug)]
pub struct Kernelspace;

pub trait AddressSpace {
    fn address_range() -> Range<VirtualAddress>;
}

impl AddressSpace for Userspace {
    fn address_range() -> Range<VirtualAddress> {
        VirtualAddress::userspace_range()
    }
}

impl AddressSpace for Kernelspace {
    fn address_range() -> Range<VirtualAddress> {
        VirtualAddress::kernelspace_range()
    }
}

/// Represents the userspace address space and allows for allocating and
/// deallocating regions of the address space
#[derive(Debug)]
pub struct AddressMap<A: AddressSpace> {
    map: BTreeMap<VirtualAddress, AddressRegion>,
    address_space: core::marker::PhantomData<A>,
}

impl<A: AddressSpace> AddressMap<A> {
    /// Create a new [`AddressMap`]
    pub fn new() -> Self {
        let complete_range = A::address_range();
        let mut map = BTreeMap::new();
        map.insert(
            complete_range.end,
            AddressRegion {
                region: None,
                span: complete_range,
                kind: AddressRegionKind::Unoccupied,
                permissions: Flags::NONE,
            },
        );

        Self { map, address_space: core::marker::PhantomData }
    }

    /// Allocate a new virtual memory region backed by the given
    /// [`MemoryRegion`] at the given range. Returns `Err(())` if the region is
    /// already occupied.
    pub fn alloc(
        &mut self,
        subrange: Range<VirtualAddress>,
        backing: MemoryRegion,
        kind: AddressRegionKind,
        permissions: Flags,
    ) -> Result<(), AddressMappingError> {
        // Safety note: we enforce that we only deal with userspace mappings
        // that never cross into the address hole, so the
        // `VirtualAddress::unchecked_offset`s are safe.
        //
        // The `unchecked_offset(-1)`s are necessary because otherwise for
        // page-aligned addresses that lie on a region boundary (e.g. we're
        // unmapping a range), we would otherwise pick up the previous range
        // which then would cause issues or panic later on.

        let key = match self.map.range(subrange.start..).next() {
            Some((_, range))
                if range.span.start > subrange.start || range.span.end < subrange.end || range.region.is_some() =>
            {
                log::trace!(
                    "Address region {:#p}-{:#p} already occupied by region {:?}",
                    subrange.start,
                    subrange.end,
                    range
                );
                return Err(AddressMappingError::Occupied);
            }
            None => return Err(AddressMappingError::OutOfBounds),
            Some((key, _)) => *key,
        };

        let mut old_range = self.map.remove(&key).unwrap();

        match (old_range.span.start == subrange.start, old_range.span.end == subrange.end) {
            // Chop off the start
            (true, false) => {
                old_range.span = subrange.end..old_range.span.end;
                self.map.insert(unsafe { old_range.span.end.unchecked_offset(-1) }, old_range);
                self.map.insert(
                    unsafe { subrange.end.unchecked_offset(-1) },
                    AddressRegion { region: Some(backing), span: subrange, kind, permissions },
                );
            }
            // Chop off the end
            (false, true) => {
                old_range.span = old_range.span.start..subrange.start;
                self.map.insert(unsafe { old_range.span.end.unchecked_offset(-1) }, old_range);
                self.map.insert(
                    unsafe { subrange.end.unchecked_offset(-1) },
                    AddressRegion { region: Some(backing), span: subrange, kind, permissions },
                );
            }
            // its the whole ass range
            (true, true) => {
                self.map.insert(
                    unsafe { subrange.end.unchecked_offset(-1) },
                    AddressRegion { region: Some(backing), span: subrange, kind, permissions },
                );
            }
            // its a true subrange, need to splice out an generate 3 new ranges
            (false, false) => {
                let before = AddressRegion {
                    region: None,
                    span: old_range.span.start..subrange.start,
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::NONE,
                };
                let active = AddressRegion { region: Some(backing), span: subrange.clone(), kind, permissions };
                let after = AddressRegion {
                    region: None,
                    span: subrange.end..old_range.span.end,
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::NONE,
                };

                self.map.insert(unsafe { before.span.end.unchecked_offset(-1) }, before);
                self.map.insert(unsafe { active.span.end.unchecked_offset(-1) }, active);
                self.map.insert(unsafe { after.span.end.unchecked_offset(-1) }, after);
            }
        }

        Ok(())
    }

    /// Free the given range, returning the backing [`MemoryRegion`] or an
    /// `Err(())` if the range wasn't occupied
    pub fn free(&mut self, range: Range<VirtualAddress>) -> Result<MemoryRegion, AddressMappingError> {
        match self.map.range(range.start..).next() {
            Some((_, curr_range))
                if curr_range.span.start != range.start
                    || curr_range.span.end != range.end
                    || curr_range.region.is_none() =>
            {
                return Err(AddressMappingError::Nonexistent);
            }
            None => return Err(AddressMappingError::OutOfBounds),
            _ => {}
        }

        let mut range = self.map.remove(&range.end.offset(-1)).unwrap();

        // Coalesce free regions around into a single region
        while let Some((&key, AddressRegion { region: None, .. })) = self.map.range(..range.span.start).next() {
            let start = self.map.remove(&key).unwrap().span.start;
            range.span.start = start;
        }

        while let Some((&key, AddressRegion { region: None, .. })) = self.map.range(range.span.end..).next() {
            let end = self.map.remove(&key).unwrap().span.end;
            range.span.end = end;
        }

        let ret = range.region.take().unwrap();

        self.map.insert(unsafe { range.span.end.unchecked_offset(-1) }, range);

        Ok(ret)
    }

    /// Find the region containing the given [`VirtualAddress`]
    pub fn find(&self, address: VirtualAddress) -> Option<&AddressRegion> {
        self.map.range(address..).next().map(|(_, r)| r)
    }

    /// Returns the unoccupied regions in the address space
    pub fn unoccupied_regions(&self) -> impl Iterator<Item = &AddressRegion> {
        self.map.values().filter(|v| v.region.is_none())
    }

    /// Returns the occupied regions in the address space
    pub fn occupied_regions(&self) -> impl Iterator<Item = &AddressRegion> {
        self.map.values().filter(|v| v.region.is_some())
    }

    pub fn debug(&self, addr: Option<VirtualAddress>) -> impl core::fmt::Debug + '_ {
        AddressMapDebug(self, addr)
    }
}

struct AddressMapDebug<'a, A: AddressSpace>(&'a AddressMap<A>, Option<VirtualAddress>);
impl<A: AddressSpace> core::fmt::Debug for AddressMapDebug<'_, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match f.alternate() {
            true => {
                let inside_region = match self.1 {
                    Some(addr) => self.0.occupied_regions().find(|r| r.span.contains(&addr)),
                    None => None,
                };

                let inbetween_regions = match self.1 {
                    Some(addr) => self
                        .0
                        .occupied_regions()
                        .zip(self.0.occupied_regions().skip(1))
                        .find(|(s, e)| (s.span.end..e.span.start).contains(&addr)),
                    None => None,
                };

                for region in self.0.occupied_regions() {
                    writeln!(
                        f,
                        "[{:?}] {:#p}..{:#p}: {:<15}({}{}{}{}{}) {}",
                        region.region.as_ref().unwrap().page_size(),
                        region.span.start,
                        region.span.end,
                        // Apparently padding doesn't work with debug printing...
                        alloc::format!("{:?}", region.kind),
                        if region.permissions & Flags::VALID { 'V' } else { '-' },
                        if region.permissions & Flags::USER { 'U' } else { 'K' },
                        if region.permissions & Flags::READ { 'R' } else { '-' },
                        if region.permissions & Flags::WRITE { 'W' } else { '-' },
                        if region.permissions & Flags::EXECUTE { 'X' } else { '-' },
                        match (inside_region, inbetween_regions) {
                            (Some(inside), _) if inside.span == region.span => "<-- fault lies inside this region",
                            (None, Some((start, _))) if start.span == region.span =>
                                "<-- fault lies inbetween this region",
                            (None, Some((_, end))) if end.span == region.span => "<-- ... and this region",
                            _ => "",
                        }
                    )?;
                }

                Ok(())
            }
            false => f.debug_struct("AddressMap").field("map", &self.0.map).finish(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddressMappingError {
    Occupied,
    Nonexistent,
    OutOfBounds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_works() {
        // Initial allocation, aka splitting into three parts
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);
        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied, Flags::READ).unwrap();

        assert_eq!(
            am.occupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![&AddressRegion {
                region: Some(MemoryRegion::GuardPage),
                span: subrange.clone(),
                kind: AddressRegionKind::Unoccupied,
                permissions: Flags::READ,
            }]
        );

        assert_eq!(
            am.unoccupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                &AddressRegion {
                    region: None,
                    span: VirtualAddress::new(0)..subrange.start,
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::USER,
                },
                &AddressRegion {
                    region: None,
                    span: subrange.end..VirtualAddress::userspace_range().end,
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::USER,
                }
            ]
        );

        // Full first-subrange allocation
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);
        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied, Flags::USER).unwrap();
        am.alloc(
            VirtualAddress::new(0)..subrange.start,
            MemoryRegion::GuardPage,
            AddressRegionKind::Unoccupied,
            Flags::USER,
        )
        .unwrap();

        assert_eq!(
            am.occupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: VirtualAddress::new(0)..subrange.start,
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::USER,
                },
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: subrange.clone(),
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::USER,
                },
            ]
        );

        assert_eq!(
            am.unoccupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![&AddressRegion {
                region: None,
                span: subrange.end..VirtualAddress::userspace_range().end,
                kind: AddressRegionKind::Unoccupied,
                permissions: Flags::USER,
            }]
        );

        // Full second-subrange allocation
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);
        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied, Flags::USER).unwrap();
        am.alloc(
            subrange.end..VirtualAddress::userspace_range().end,
            MemoryRegion::GuardPage,
            AddressRegionKind::Unoccupied,
            Flags::USER,
        )
        .unwrap();

        assert_eq!(
            am.occupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: subrange.clone(),
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::USER,
                },
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: subrange.end..VirtualAddress::userspace_range().end,
                    kind: AddressRegionKind::Unoccupied,
                    permissions: Flags::USER,
                }
            ]
        );

        assert_eq!(
            am.unoccupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![&AddressRegion {
                region: None,
                span: VirtualAddress::new(0)..subrange.start,
                kind: AddressRegionKind::Unoccupied,
                permissions: Flags::USER,
            }]
        );
    }

    #[test]
    fn coalesce_works() {
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);

        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied, Flags::USER).unwrap();
        am.free(subrange).unwrap();

        assert_eq!(
            am.unoccupied_regions().next().unwrap(),
            &AddressRegion {
                region: None,
                span: VirtualAddress::userspace_range(),
                kind: AddressRegionKind::Unoccupied,
                permissions: Flags::USER,
            }
        );
    }

    #[test]
    fn doesnt_grab_existing_region() {
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);

        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied, Flags::USER).unwrap();
        am.alloc(
            VirtualAddress::new(0x4_0000)..VirtualAddress::new(0x5_0000),
            MemoryRegion::GuardPage,
            AddressRegionKind::Unoccupied,
            Flags::USER,
        )
        .unwrap_err();
        am.free(subrange).unwrap();

        assert_eq!(
            am.unoccupied_regions().next().unwrap(),
            &AddressRegion {
                region: None,
                span: VirtualAddress::userspace_range(),
                kind: AddressRegionKind::Unoccupied,
                permissions: Flags::USER,
            }
        );
    }
}
