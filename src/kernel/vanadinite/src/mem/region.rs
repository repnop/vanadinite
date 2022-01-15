// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{paging::PageSize, PhysicalAddress};
use crate::mem::{
    phys::{PhysicalMemoryAllocator, PhysicalPage, PHYSICAL_MEMORY_ALLOCATOR},
    phys2virt,
};
use alloc::{sync::Arc, vec::Vec};

#[derive(Debug, PartialEq)]
pub enum MemoryRegion {
    Backed(PhysicalRegion),
    Lazy { page_size: PageSize, n_pages: usize },
    GuardPage,
}

impl MemoryRegion {
    pub fn page_size(&self) -> PageSize {
        match self {
            MemoryRegion::GuardPage => PageSize::Kilopage,
            MemoryRegion::Lazy { page_size, .. } => *page_size,
            MemoryRegion::Backed(backing) => backing.page_size(),
        }
    }

    pub fn page_count(&self) -> usize {
        match self {
            MemoryRegion::GuardPage => 1,
            MemoryRegion::Lazy { n_pages, .. } => *n_pages,
            MemoryRegion::Backed(backing) => backing.page_count(),
        }
    }
}

#[derive(Debug, PartialEq)]
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

    pub fn page_size(&self) -> PageSize {
        match self {
            PhysicalRegion::Shared(shared) => shared.page_size,
            PhysicalRegion::Unique(unique) => unique.page_size,
        }
    }
}

#[derive(Debug, PartialEq)]
enum PhysicalRegionKind {
    Contiguous(PhysicalPage),
    Mmio(PhysicalPage),
    Sparse(Vec<PhysicalPage>),
}

#[derive(Debug, PartialEq)]
pub struct UniquePhysicalRegion {
    kind: PhysicalRegionKind,
    page_size: PageSize,
    n_pages: usize,
}

impl UniquePhysicalRegion {
    /// This function allows aliasing physical memory at arbitrary addresses,
    /// bypassing the physical frame allocator.
    #[track_caller]
    pub fn mmio(at: PhysicalAddress, page_size: PageSize, n_pages: usize) -> Self {
        Self { kind: PhysicalRegionKind::Mmio(PhysicalPage::from_ptr(at.as_mut_ptr())), page_size, n_pages }
    }

    #[track_caller]
    pub fn alloc_contiguous(page_size: PageSize, n_pages: usize) -> Self {
        log::debug!("Allocating page for contiguous region");
        let mut lock = PHYSICAL_MEMORY_ALLOCATOR.lock();

        let kind = PhysicalRegionKind::Contiguous(unsafe {
            lock.alloc_contiguous(page_size, n_pages).expect("couldn't alloc contiguous region")
        });

        Self { kind, page_size, n_pages }
    }

    #[track_caller]
    pub fn alloc_sparse(page_size: PageSize, n_pages: usize) -> Self {
        if n_pages == 1 {
            return Self::alloc_contiguous(page_size, 1);
        }

        let kind = PhysicalRegionKind::Sparse(unsafe {
            let mut allocator = PHYSICAL_MEMORY_ALLOCATOR.lock();
            let mut pages = Vec::with_capacity(n_pages);

            for _ in 0..n_pages {
                log::trace!("Allocating page for sparse region");
                pages.push(allocator.alloc(page_size).expect("couldn't alloc sparse region"));
            }

            pages
        });

        Self { kind, page_size, n_pages }
    }

    pub fn physical_addresses(&self) -> impl Iterator<Item = PhysicalAddress> + '_ {
        let contig = match &self.kind {
            PhysicalRegionKind::Contiguous(start) | PhysicalRegionKind::Mmio(start) => {
                Some((0..self.n_pages).map(move |i| start.as_phys_address().offset(i * self.page_size.to_byte_size())))
            }
            PhysicalRegionKind::Sparse(_) => None,
        };

        let sparse = match &self.kind {
            PhysicalRegionKind::Sparse(pages) => Some(pages.iter().map(|p| p.as_phys_address())),
            PhysicalRegionKind::Contiguous(_) | PhysicalRegionKind::Mmio(_) => None,
        };

        contig.into_iter().flatten().chain(sparse.into_iter().flatten())
    }

    pub fn copy_data_into(&mut self, data: &[u8]) {
        for (phys_addr, data) in self.physical_addresses().zip(data.chunks(self.page_size.to_byte_size())) {
            let copy_to = unsafe {
                core::slice::from_raw_parts_mut(phys2virt(phys_addr).as_mut_ptr(), self.page_size.to_byte_size())
            };

            copy_to[..data.len()].copy_from_slice(data);
        }
    }

    pub fn zero(&mut self) {
        for phys_addr in self.physical_addresses() {
            let copy_to = unsafe {
                core::slice::from_raw_parts_mut(phys2virt(phys_addr).as_mut_ptr(), self.page_size.to_byte_size())
            };

            copy_to.fill(0);
        }
    }

    pub fn into_shared_region(self) -> SharedPhysicalRegion {
        SharedPhysicalRegion { region: Arc::new(self) }
    }

    pub fn page_size(&self) -> PageSize {
        self.page_size
    }

    pub fn n_pages(&self) -> usize {
        self.n_pages
    }
}

impl Drop for UniquePhysicalRegion {
    fn drop(&mut self) {
        match &mut self.kind {
            PhysicalRegionKind::Contiguous(start) => unsafe {
                PHYSICAL_MEMORY_ALLOCATOR.lock().dealloc_contiguous(*start, self.page_size, self.n_pages)
            },
            PhysicalRegionKind::Sparse(pages) => {
                let mut allocator = PHYSICAL_MEMORY_ALLOCATOR.lock();

                for page in pages.drain(..) {
                    unsafe { allocator.dealloc(page, self.page_size) };
                }
            }
            // These are directly mapped, so we don't need to deallocate pages
            PhysicalRegionKind::Mmio(_) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedPhysicalRegion {
    region: Arc<UniquePhysicalRegion>,
}

impl core::ops::Deref for SharedPhysicalRegion {
    type Target = UniquePhysicalRegion;

    fn deref(&self) -> &Self::Target {
        &self.region
    }
}
