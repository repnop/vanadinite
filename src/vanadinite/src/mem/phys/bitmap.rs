// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{PhysicalAddress, PhysicalMemoryAllocator, PhysicalPage};
use crate::Units;

const SINGLE_ENTRY_SIZE_BYTES: usize = 64 * 4096;
const FULL_ENTRY: u64 = 0xFFFF_FFFF_FFFF_FFFF;

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
    unsafe fn alloc(&mut self) -> Option<PhysicalPage> {
        if let Some((index, entry)) = self.bitmap_slice().iter_mut().enumerate().find(|(_, e)| **e != FULL_ENTRY) {
            let bit_index = entry.trailing_ones() as usize;

            let page_ptr = (self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES) + (bit_index * 4096);
            let page_ptr = page_ptr as *mut u8;

            if page_ptr <= self.mem_end {
                *entry |= 1 << bit_index;
                log::debug!("Allocated page at: {:#p}", page_ptr);
                return Some(PhysicalPage::from_ptr(page_ptr));
            }
        }

        None
    }

    // FIXME: this should look for inter-u64 regions
    unsafe fn alloc_contiguous(&mut self, n: usize) -> Option<PhysicalPage> {
        assert!(n <= 64, "> 64 page allocations are currently not supported");
        let mask = u64::max_value() << n;
        for (index, entry) in self.bitmap_slice().iter_mut().enumerate().filter(|(_, e)| e.count_zeros() as usize >= n)
        {
            if n == 64 {
                *entry = u64::max_value();
                let page_ptr = self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES;
                let page_ptr = page_ptr as *mut u8;

                if page_ptr >= self.mem_end {
                    return None;
                }

                return Some(PhysicalPage::from_ptr(page_ptr));
            }

            let mut bit_index = None;
            for i in 0..=(64 - n as u64) {
                let selected = *entry | mask.rotate_left(i as u32);
                let shifted = selected >> i;

                if !shifted & !mask == !mask {
                    bit_index = Some(i as usize);
                    break;
                }
            }

            let bit_index = match bit_index {
                Some(b) => b,
                None => continue,
            };

            let page_ptr = (self.mem_start as usize + index * SINGLE_ENTRY_SIZE_BYTES) + (bit_index * 4096);
            let page_ptr = page_ptr as *mut u8;

            if page_ptr >= self.mem_end {
                return None;
            }

            let page = Some(PhysicalPage::from_ptr(page_ptr));
            *entry |= (!mask).rotate_left(bit_index as u32);

            return page;
        }

        None
    }

    #[track_caller]
    unsafe fn dealloc(&mut self, page: PhysicalPage) {
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

    #[track_caller]
    unsafe fn dealloc_contiguous(&mut self, page: PhysicalPage, n: usize) {
        let index = (page.as_phys_address().as_usize() - self.mem_start as usize) / SINGLE_ENTRY_SIZE_BYTES;
        let bit = ((page.as_phys_address().as_usize() - self.mem_start as usize) / 4096) % 64;
        let mask = u64::max_value() << n;

        let entry = &mut self.bitmap_slice()[index];

        if (*entry >> bit) & !mask != !mask {
            panic!(
                "[pmalloc.allocator] BitmapAllocator::dealloc: double free detected for address {:#p}",
                page.as_phys_address().as_ptr()
            );
        }

        *entry &= mask.rotate_left(bit as u32);
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
