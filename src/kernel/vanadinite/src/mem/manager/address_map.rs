// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::VirtualAddress;
use crate::mem::region::MemoryRegion;
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
}

/// Represents the userspace address space and allows for allocating and
/// deallocating regions of the address space
pub struct AddressMap {
    map: BTreeMap<VirtualAddress, AddressRegion>,
}

impl AddressMap {
    /// Create a new [`AddressMap`]
    pub fn new() -> Self {
        let complete_range = VirtualAddress::userspace_range();
        let mut map = BTreeMap::new();
        map.insert(
            complete_range.end,
            AddressRegion { region: None, span: complete_range, kind: AddressRegionKind::Unoccupied },
        );

        Self { map }
    }

    /// Allocate a new virtual memory region backed by the given
    /// [`MemoryRegion`] at the given range. Returns `Err(())` if the region is
    /// already occupied.
    pub fn alloc(
        &mut self,
        subrange: Range<VirtualAddress>,
        backing: MemoryRegion,
        kind: AddressRegionKind,
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
                    AddressRegion { region: Some(backing), span: subrange, kind },
                );
            }
            // Chop off the end
            (false, true) => {
                old_range.span = old_range.span.start..subrange.start;
                self.map.insert(unsafe { old_range.span.end.unchecked_offset(-1) }, old_range);
                self.map.insert(
                    unsafe { subrange.end.unchecked_offset(-1) },
                    AddressRegion { region: Some(backing), span: subrange, kind },
                );
            }
            // its the whole ass range
            (true, true) => {
                self.map.insert(
                    unsafe { subrange.end.unchecked_offset(-1) },
                    AddressRegion { region: Some(backing), span: subrange, kind },
                );
            }
            // its a true subrange, need to splice out an generate 3 new ranges
            (false, false) => {
                let before = AddressRegion {
                    region: None,
                    span: old_range.span.start..subrange.start,
                    kind: AddressRegionKind::Unoccupied,
                };
                let active = AddressRegion { region: Some(backing), span: subrange.clone(), kind };
                let after = AddressRegion {
                    region: None,
                    span: subrange.end..old_range.span.end,
                    kind: AddressRegionKind::Unoccupied,
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
}

impl core::fmt::Debug for AddressMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match f.alternate() {
            true => {
                for region in self.occupied_regions() {
                    writeln!(
                        f,
                        "[{:?}] {:#p}..{:#p}: {:?}",
                        region.region.as_ref().unwrap().page_size(),
                        region.span.start,
                        region.span.end,
                        region.kind,
                    )?;
                }

                Ok(())
            }
            false => f.debug_struct("AddressMap").field("map", &self.map).finish(),
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
        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied).unwrap();

        assert_eq!(
            am.occupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![&AddressRegion {
                region: Some(MemoryRegion::GuardPage),
                span: subrange.clone(),
                kind: AddressRegionKind::Unoccupied,
            }]
        );

        assert_eq!(
            am.unoccupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                &AddressRegion {
                    region: None,
                    span: VirtualAddress::new(0)..subrange.start,
                    kind: AddressRegionKind::Unoccupied
                },
                &AddressRegion {
                    region: None,
                    span: subrange.end..VirtualAddress::userspace_range().end,
                    kind: AddressRegionKind::Unoccupied,
                }
            ]
        );

        // Full first-subrange allocation
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);
        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied).unwrap();
        am.alloc(VirtualAddress::new(0)..subrange.start, MemoryRegion::GuardPage, AddressRegionKind::Unoccupied)
            .unwrap();

        assert_eq!(
            am.occupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: VirtualAddress::new(0)..subrange.start,
                    kind: AddressRegionKind::Unoccupied,
                },
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: subrange.clone(),
                    kind: AddressRegionKind::Unoccupied,
                },
            ]
        );

        assert_eq!(
            am.unoccupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![&AddressRegion {
                region: None,
                span: subrange.end..VirtualAddress::userspace_range().end,
                kind: AddressRegionKind::Unoccupied,
            }]
        );

        // Full second-subrange allocation
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);
        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied).unwrap();
        am.alloc(
            subrange.end..VirtualAddress::userspace_range().end,
            MemoryRegion::GuardPage,
            AddressRegionKind::Unoccupied,
        )
        .unwrap();

        assert_eq!(
            am.occupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: subrange.clone(),
                    kind: AddressRegionKind::Unoccupied,
                },
                &AddressRegion {
                    region: Some(MemoryRegion::GuardPage),
                    span: subrange.end..VirtualAddress::userspace_range().end,
                    kind: AddressRegionKind::Unoccupied,
                }
            ]
        );

        assert_eq!(
            am.unoccupied_regions().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![&AddressRegion {
                region: None,
                span: VirtualAddress::new(0)..subrange.start,
                kind: AddressRegionKind::Unoccupied,
            }]
        );
    }

    #[test]
    fn coalesce_works() {
        let mut am = AddressMap::new();
        let subrange = VirtualAddress::new(0x1_0000)..VirtualAddress::new(0x5_0000);

        am.alloc(subrange.clone(), MemoryRegion::GuardPage, AddressRegionKind::Unoccupied).unwrap();
        am.free(subrange).unwrap();

        assert_eq!(
            am.unoccupied_regions().next().unwrap(),
            &AddressRegion {
                region: None,
                span: VirtualAddress::userspace_range(),
                kind: AddressRegionKind::Unoccupied,
            }
        );
    }
}
