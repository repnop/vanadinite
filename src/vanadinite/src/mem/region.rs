// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::PhysicalAddress;
use crate::{
    mem::{
        phys::{PhysicalMemoryAllocator, PhysicalPage, PHYSICAL_MEMORY_ALLOCATOR},
        phys2virt,
    },
    utils::Units,
};
use alloc::{sync::Arc, vec::Vec};

#[derive(Debug)]
pub enum PhysicalRegion {
    Shared(SharedPhysicalRegion),
    Unique(UniquePhysicalRegion),
}

impl PhysicalRegion {
    /// Returns the number of pages contained within the region
    pub fn page_count(&self) -> usize {
        match self {
            PhysicalRegion::Shared(shared) => shared.n_pages,
            PhysicalRegion::Unique(unique) => unique.n_pages,
        }
    }

    pub fn physical_addresses(&self) -> impl Iterator<Item = PhysicalAddress> + '_ {
        match self {
            PhysicalRegion::Shared(shared) => shared.physical_addresses(),
            PhysicalRegion::Unique(unique) => unique.physical_addresses(),
        }
    }
}

#[derive(Debug)]
enum PhysicalRegionKind {
    Contiguous(PhysicalPage),
    Sparse(Vec<PhysicalPage>),
}

#[derive(Debug)]
pub struct UniquePhysicalRegion {
    kind: PhysicalRegionKind,
    n_pages: usize,
}

impl UniquePhysicalRegion {
    #[track_caller]
    pub fn alloc_contiguous(n_pages: usize) -> Self {
        let kind = PhysicalRegionKind::Contiguous(unsafe {
            PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(n_pages).expect("couldn't alloc contiguous region")
        });

        Self { kind, n_pages }
    }

    #[track_caller]
    pub fn alloc_sparse(n_pages: usize) -> Self {
        if n_pages == 1 {
            return Self::alloc_contiguous(1);
        }

        let kind = PhysicalRegionKind::Sparse(unsafe {
            let mut allocator = PHYSICAL_MEMORY_ALLOCATOR.lock();
            let mut pages = Vec::with_capacity(n_pages);

            for _ in 0..n_pages {
                pages.push(allocator.alloc().expect("couldn't alloc sparse region"));
            }

            pages
        });

        Self { kind, n_pages }
    }

    pub fn physical_addresses(&self) -> impl Iterator<Item = PhysicalAddress> + '_ {
        let contig = match &self.kind {
            PhysicalRegionKind::Contiguous(start) => {
                Some((0..self.n_pages).map(move |i| start.as_phys_address().offset(i * 4.kib())))
            }
            PhysicalRegionKind::Sparse(_) => None,
        };

        let sparse = match &self.kind {
            PhysicalRegionKind::Sparse(pages) => Some(pages.iter().map(|p| p.as_phys_address())),
            PhysicalRegionKind::Contiguous(_) => None,
        };

        contig.into_iter().flatten().chain(sparse.into_iter().flatten())
    }

    pub fn copy_data_into(&mut self, data: &[u8]) {
        for (phys_addr, data) in self.physical_addresses().zip(data.chunks(4.kib())) {
            let copy_to = unsafe { core::slice::from_raw_parts_mut(phys2virt(phys_addr).as_mut_ptr(), 4.kib()) };

            copy_to[..data.len()].copy_from_slice(data);
        }
    }

    pub fn into_shared_region(self) -> SharedPhysicalRegion {
        SharedPhysicalRegion { region: Arc::new(self) }
    }
}

impl Drop for UniquePhysicalRegion {
    fn drop(&mut self) {
        match &mut self.kind {
            PhysicalRegionKind::Contiguous(start) => unsafe {
                PHYSICAL_MEMORY_ALLOCATOR.lock().dealloc_contiguous(*start, self.n_pages)
            },
            PhysicalRegionKind::Sparse(pages) => {
                let mut allocator = PHYSICAL_MEMORY_ALLOCATOR.lock();

                for page in pages.drain(..) {
                    unsafe { allocator.dealloc(page) };
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SharedPhysicalRegion {
    region: Arc<UniquePhysicalRegion>,
}

impl core::ops::Deref for SharedPhysicalRegion {
    type Target = UniquePhysicalRegion;

    fn deref(&self) -> &Self::Target {
        &self.region
    }
}
