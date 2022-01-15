// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{PhysicalAddress, PhysicalMemoryAllocator, PhysicalPage};
use crate::{mem::paging::PageSize, Units};

const SINGLE_ENTRY_SIZE_BYTES: usize = 64 * 4096;

pub struct BitmapAllocator {
    bitmap: *mut u64,
    size: usize,
    mem_start: *mut u8,
    mem_end: *mut u8,
}

impl BitmapAllocator {
    pub const fn new() -> Self {
        Self {
            bitmap: core::ptr::null_mut(),
            size: 0,
            mem_start: core::ptr::null_mut(),
            mem_end: core::ptr::null_mut(),
        }
    }

    fn bitmap_slice(&mut self) -> &'static mut [u64] {
        unsafe {
            core::slice::from_raw_parts_mut(
                crate::mem::phys2virt(PhysicalAddress::from_ptr(self.bitmap)).as_mut_ptr().cast(),
                self.size,
            )
        }
    }

    // TODO: Check for small inter-regions as well
    fn alloc_contig_4k_intra_pages(&mut self, n: usize) -> Option<PhysicalPage> {
        let mask = u64::MAX << n;

        if n == 64 {
            let (index, entry) = self.bitmap_slice().iter_mut().enumerate().find(|(_, e)| **e == 0)?;
            *entry = u64::MAX;

            let page_ptr = (self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES) as *mut u8;

            return match page_ptr < self.mem_end {
                true => Some(PhysicalPage::from_ptr(page_ptr as *mut u8)),
                false => None,
            };
        }

        let free_bit_filter = |(_, e): &(usize, &mut u64)| e.count_zeros() as usize >= n;
        for (index, entry) in self.bitmap_slice().iter_mut().enumerate().filter(free_bit_filter) {
            let bit_index = match (0..(64 - n)).map(|i| (i, *entry >> i)).find(|(_, e)| e | mask == mask) {
                Some((idx, _)) => idx,
                None => continue,
            };

            *entry |= (!mask).rotate_left(bit_index as u32);

            let page_ptr = (self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES) + (bit_index * 4096);
            let page_ptr = page_ptr as *mut u8;

            if page_ptr < self.mem_end {
                return Some(PhysicalPage::from_ptr(page_ptr));
            }
        }

        None
    }

    fn alloc_contig_4k_inter_pages(&mut self, n: usize) -> Option<PhysicalPage> {
        let whole_entries_needed = n / 64;
        let last_bits_needed = (n % 64) as u32;

        let mut start_index = 0;
        let bitmap = self.bitmap_slice();

        loop {
            let range = start_index..(start_index + whole_entries_needed);

            if bitmap.get(range.clone())?.iter().any(|e| *e != 0) {
                start_index += whole_entries_needed;
                continue;
            }

            if last_bits_needed != 0 && bitmap.get(range.end)?.leading_zeros() < last_bits_needed {
                start_index = range.end + 1;
                continue;
            }

            let page_ptr = self.mem_start as usize + start_index * SINGLE_ENTRY_SIZE_BYTES;
            let page_ptr = page_ptr as *mut u8;

            if page_ptr < self.mem_end {
                bitmap.get_mut(range.clone())?.iter_mut().for_each(|e| *e = u64::MAX);
                if last_bits_needed > 0 {
                    *bitmap.get_mut(range.end)? |= !(u64::MAX << last_bits_needed);
                }

                return Some(PhysicalPage::from_ptr(page_ptr));
            } else {
                return None;
            }
        }
    }
}

unsafe impl PhysicalMemoryAllocator for BitmapAllocator {
    unsafe fn init(&mut self, start: *mut u8, end: *mut u8) {
        assert!(!start.is_null(), "null start pointer!");
        assert_eq!(start as usize % 4096, 0, "unaligned memory start page");
        self.mem_start = start;
        self.mem_end = end;

        let n_pages = (end as usize - start as usize) / 4.kib();
        self.bitmap = self.mem_start.cast();
        self.size = n_pages / 64 + 1;

        self.bitmap_slice().fill_with(|| 0);

        for page in 0..(self.size / 4.kib() + 1) {
            self.set_used(PhysicalPage::from_ptr(self.mem_start.add(4.kib() * page)));
        }
    }

    #[track_caller]
    unsafe fn alloc(&mut self, align_to: PageSize) -> Option<PhysicalPage> {
        match align_to {
            PageSize::Megapage => self.alloc_contiguous(align_to, 1),
            PageSize::Kilopage => {
                log::trace!("attempting to allocate a single page");
                if let Some((index, entry)) = self.bitmap_slice().iter_mut().enumerate().find(|(_, e)| **e != u64::MAX)
                {
                    let bit_index = entry.trailing_ones() as usize;

                    let page_ptr = (self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES) + (bit_index * 4096);
                    let page_ptr = page_ptr as *mut u8;

                    if page_ptr <= self.mem_end {
                        *entry |= 1 << bit_index;
                        log::trace!("Allocated page at: {:#p}", page_ptr);
                        return Some(PhysicalPage::from_ptr(page_ptr));
                    }
                }

                None
            }
            _ => todo!("[pmalloc.allocator] BitmapAllocator::alloc: >megapage alloc"),
        }
    }

    #[track_caller]
    unsafe fn alloc_contiguous(&mut self, align_to: PageSize, n: usize) -> Option<PhysicalPage> {
        if let PageSize::Kilopage = align_to {
            match n {
                0..=64 => return self.alloc_contig_4k_intra_pages(n),
                _ => return self.alloc_contig_4k_inter_pages(n),
            }
        }

        // Megapages and above can use the same code
        let n_entries = (((align_to.to_byte_size() / 4.kib()) * n) / 64).max(1);
        let mut start_index = {
            let offset = self.mem_start.align_offset(align_to.to_byte_size());
            offset / 64.kib()
        };

        let mut end_index = start_index + n_entries;
        while self.bitmap_slice().get(start_index..end_index)?.iter().any(|n| n.count_ones() != 0) {
            start_index += n_entries;
            end_index = start_index + n_entries;
        }

        for entry in &mut self.bitmap_slice()[start_index..end_index] {
            *entry = u64::MAX;
        }

        let page_ptr = self.mem_start as usize + start_index * SINGLE_ENTRY_SIZE_BYTES;
        assert_eq!(page_ptr % align_to.to_byte_size(), 0);
        Some(PhysicalPage::from_ptr(page_ptr as *mut u8))
    }

    #[track_caller]
    unsafe fn dealloc(&mut self, page: PhysicalPage, size: PageSize) {
        match size {
            PageSize::Megapage => {
                let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
                for entry in &mut self.bitmap_slice()[index..][..512] {
                    assert_eq!(
                        *entry,
                        u64::MAX,
                        "[pmalloc.allocator] BitmapAllocator::dealloc: double free in large page region!"
                    );
                    *entry = 0;
                }
            }
            PageSize::Kilopage => {
                let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
                let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;

                let entry = &mut self.bitmap_slice()[index];

                if (*entry >> bit) & 1 != 1 {
                    panic!(
                        "[pmalloc.allocator] BitmapAllocator::dealloc: double free detected for address {:#p}",
                        page.as_phys_address().as_ptr()
                    );
                }

                *entry &= !(1 << bit);
            }
            _ => todo!("[pmalloc.allocator] BitmapAllocator::dealloc: >megapage dealloc"),
        }
    }

    #[track_caller]
    unsafe fn dealloc_contiguous(&mut self, page: PhysicalPage, size: PageSize, n: usize) {
        let start_index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;

        match size {
            PageSize::Kilopage => match n {
                0..=64 => {
                    let start_bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4.kib()) % 64;

                    if n == 64 {
                        self.bitmap_slice()[start_index] = 0;
                    } else {
                        let mask = (u64::MAX << n).rotate_left(start_bit as u32);
                        self.bitmap_slice()[start_index] &= mask;
                    }
                }
                _ => {
                    let whole_entries_needed = n / 64;
                    let last_bits_needed = (n % 64) as u32;
                    let range = start_index..(start_index + whole_entries_needed);

                    for entry in &mut self.bitmap_slice()[range.clone()] {
                        assert_eq!(
                            *entry,
                            u64::MAX,
                            "[pmalloc.allocator] BitmapAllocator::dealloc: double free in contiguous page region!"
                        );
                        *entry = 0;
                    }

                    if last_bits_needed > 0 {
                        self.bitmap_slice()[range.end] &= u64::MAX << last_bits_needed;
                    }
                }
            },
            _ => {
                let n_entries = (((size.to_byte_size() / 4.kib()) * n) / 64).max(1);
                let end_index = start_index + n_entries;

                for entry in &mut self.bitmap_slice()[start_index..][..end_index] {
                    assert_eq!(
                        *entry,
                        u64::MAX,
                        "[pmalloc.allocator] BitmapAllocator::dealloc: double free in contiguous page region!"
                    );
                    *entry = 0;
                }
            }
        }
    }

    #[track_caller]
    unsafe fn set_used(&mut self, page: PhysicalPage) {
        let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
        let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;

        let entry = &mut self.bitmap_slice()[index];

        *entry |= 1 << bit;
    }

    #[track_caller]
    unsafe fn set_unused(&mut self, page: PhysicalPage) {
        let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
        let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;

        let entry = &mut self.bitmap_slice()[index];

        *entry &= !(1 << bit);
    }
}

unsafe impl Send for BitmapAllocator {}
unsafe impl Sync for BitmapAllocator {}
