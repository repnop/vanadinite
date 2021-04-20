// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{mem::paging::table::flags::*, utils::Units};

// Default to Sv39
#[cfg(all(not(feature = "paging.sv48"), not(feature = "paging.sv57")))]
const N_VPN: usize = 3;
// Sv48
#[cfg(all(feature = "paging.sv48", not(feature = "paging.sv57")))]
const N_VPN: usize = 4;
// Sv57
#[cfg(feature = "paging.sv57")]
const N_VPN: usize = 5;

const VPN_BITMASK: usize = 0x1FF;
const PPN_MASK: usize = 0x00FF_FFFF_FFFF_FFFF;

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl Default for PageTable {
    fn default() -> Self {
        Self { entries: [PageTableEntry::default(); 512] }
    }
}

#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

// TODO: ensure upper bits are zeroed?
impl PageTableEntry {
    pub const fn new() -> Self {
        PageTableEntry(0)
    }

    pub fn is_valid(self) -> bool {
        self.0 & 1 == 1
    }

    pub fn flags(self) -> Flags {
        Flags::new(self.0 as u8)
    }

    pub fn set_flags(&mut self, flags: Flags) {
        let this = self.0 & !(0xFF);
        self.0 = this | flags.value() as u64;
    }

    pub fn rsw(self) -> u8 {
        ((self.0 >> 8) & 0b11) as u8
    }

    pub fn set_rsw(&mut self, bits: u8) {
        let this = self.0 & !(0x3 << 8);
        self.0 = this | (bits & 0x3) as u64;
    }

    pub fn ppn(self) -> Option<PhysicalAddress> {
        if !self.is_valid() {
            return None;
        }

        Some(PhysicalAddress::new(((self.0 >> 10) << 12) as usize))
    }

    pub fn set_ppn(&mut self, address: PhysicalAddress) {
        let address = (address.ppn() << 10) as u64;
        let this = self.0 & 0x3FF;
        self.0 = this | address;
    }

    pub fn kind(self) -> EntryKind {
        let flags = self.flags();

        match flags & VALID {
            false => EntryKind::NotValid,
            true => match flags & READ || flags & EXECUTE {
                true => EntryKind::Leaf,
                false => EntryKind::Branch(self.ppn().unwrap()),
            },
        }
    }
}

pub enum EntryKind {
    NotValid,
    Leaf,
    Branch(PhysicalAddress),
}

impl core::fmt::Pointer for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&(self.0 as *const u8), f)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    pub const fn new(addr: usize) -> Self {
        VirtualAddress(addr)
    }

    pub fn offset(self, bytes: usize) -> Self {
        Self(self.0 + bytes)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self(ptr as usize)
    }

    pub fn as_ptr(self) -> *const u8 {
        self.0 as *const u8
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn as_mut_ptr(self) -> *mut u8 {
        self.0 as *mut u8
    }

    pub fn vpns(self) -> [usize; N_VPN] {
        #[cfg(feature = "paging.sv57")]
        compile_error!("sv57 stuff");

        let mut vpns = [0; N_VPN];
        let mut shift = 12;

        for vpn in vpns.iter_mut() {
            *vpn = (self.0 >> shift) & VPN_BITMASK;
            shift += 9;
        }

        vpns
    }

    pub fn offset_into_page(self, page_size: PageSize) -> usize {
        match page_size {
            #[cfg(any(feature = "paging.sv48", feature = "paging.sv57"))]
            PageSize::Terapage => self.0 & (512.gib() - 1),
            PageSize::Gigapage => self.0 & (1.gib() - 1),
            PageSize::Megapage => self.0 & (2.mib() - 1),
            PageSize::Kilopage => self.0 & (4.kib() - 1),
        }
    }

    pub fn from_vpns(vpns: [usize; N_VPN]) -> Self {
        #[cfg(feature = "paging.sv57")]
        compile_error!("sv57 stuff");

        let mut addr = 0;
        let mut shift = 12;

        for vpn in core::array::IntoIter::new(vpns).rev() {
            addr |= vpn << shift;
            shift += 9;
        }

        let top_most_bit = 1 << (12 + N_VPN * 9);
        if addr & top_most_bit == top_most_bit {
            addr |= usize::max_value() << (12 + N_VPN * 9);
        }

        VirtualAddress(addr)
    }

    pub fn is_kernel_region(self) -> bool {
        (self.0 as isize).is_negative()
    }
}

impl core::fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtualAddress({:#p})", self.0 as *const u8)
    }
}

impl core::fmt::Pointer for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&(self.0 as *const u8), f)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(addr: usize) -> Self {
        PhysicalAddress(addr)
    }

    pub fn offset(self, bytes: usize) -> Self {
        Self(self.0 + bytes)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self(ptr as usize)
    }

    pub fn as_ptr(self) -> *const u8 {
        self.0 as *const u8
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn as_mut_ptr(self) -> *mut u8 {
        self.0 as *mut u8
    }

    pub fn ppns(self) -> [usize; 3] {
        const PPN01_BITMASK: usize = 0x1FF;
        const PPN2_BITMASK: usize = 0x3FFFFFF;

        [(self.0 >> 12) & PPN01_BITMASK, (self.0 >> 21) & PPN01_BITMASK, (self.0 >> 30) & PPN2_BITMASK]
    }

    /// Returns the 44-bit physical page number shifted down
    pub fn ppn(self) -> usize {
        // Physical page numbers are 44 bits wide
        (self.0 >> 12) & PPN_MASK
    }

    pub fn offset_into_page(self, page_size: PageSize) -> usize {
        self.0 & (page_size.to_byte_size() - 1)
    }
}

impl core::fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysicalAddress({:#p})", self.0 as *const u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(usize)]
pub enum PageSize {
    Kilopage = 0,
    Megapage = 1,
    Gigapage = 2,
    #[cfg(any(feature = "paging.sv48", feature = "paging.sv57"))]
    Terapage = 3,
}

impl PageSize {
    #[track_caller]
    pub fn assert_addr_aligned(self, addr: usize) {
        let alignment_required = self.to_byte_size();

        assert_eq!(addr % alignment_required, 0, "physical address alignment check failed");
    }

    pub fn to_byte_size(self) -> usize {
        match self {
            PageSize::Kilopage => 4.kib(),
            PageSize::Megapage => 2.mib(),
            PageSize::Gigapage => 1.gib(),
            #[cfg(any(feature = "paging.sv48", feature = "paging.sv57"))]
            PageSize::Terapage => 512.gib(),
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            PageSize::Kilopage => None,
            PageSize::Megapage => Some(PageSize::Kilopage),
            PageSize::Gigapage => Some(PageSize::Megapage),
            #[cfg(any(feature = "paging.sv48", feature = "paging.sv57"))]
            PageSize::Terapage => Some(PageSize::Gigapage),
        }
    }

    pub fn top_level() -> Self {
        // Default to Sv39
        #[cfg(all(not(feature = "paging.sv48"), not(feature = "paging.sv57")))]
        return PageSize::Gigapage;
        // Sv48
        #[cfg(all(feature = "paging.sv48", not(feature = "paging.sv57")))]
        return PageSize::Terapage;
        // Sv57
        #[cfg(feature = "paging.sv57")]
        compile_error!("sv57 stuff");
    }
}
