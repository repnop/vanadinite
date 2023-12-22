// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ops::Range;

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

    pub fn rsw(self) -> Rsw {
        Rsw(((self.0 >> 8) & 0b11) as u8)
    }

    pub fn set_rsw(&mut self, rsw: Rsw) {
        let this = self.0 & !(0x3 << 8);
        self.0 = this | (rsw.0 & 0x3) as u64;
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

        match flags & Flags::VALID {
            false => EntryKind::NotValid,
            true => match flags & Flags::READ || flags & Flags::EXECUTE {
                true => EntryKind::Leaf,
                false => EntryKind::Branch(self.ppn().unwrap()),
            },
        }
    }
}

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PageTableEntry(")?;
        match self.kind() {
            EntryKind::Leaf => {
                write!(f, "Leaf, flags={:?}, ppn={:?}", self.flags(), self.ppn())?;
            }
            EntryKind::NotValid => {
                write!(f, "NotValid")?;
            }
            EntryKind::Branch(next_level) => {
                write!(f, "Branch, next_level={next_level:#p}")?;
            }
        }
        write!(f, ")")
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
#[repr(transparent)]
pub struct Rsw(u8);

impl Rsw {
    pub const NONE: Self = Self(0);
    pub const SHARED_MEMORY: Self = Self(1);
    pub const DIRECT: Self = Self(2);
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    pub const fn new(mut addr: usize) -> Self {
        let top_most_bit = 1 << (12 + N_VPN * 9 - 1);
        if addr & top_most_bit == top_most_bit {
            addr |= usize::MAX << (12 + N_VPN * 9 - 1);
        }
        VirtualAddress(addr)
    }

    #[allow(clippy::should_implement_trait)]
    #[must_use]
    #[track_caller]
    pub fn add(self, bytes: usize) -> Self {
        match self.checked_add(bytes) {
            Some(address) => address,
            None => panic!("invalid virtual address: self={:#p}, bytes={}", self, bytes),
        }
    }

    pub fn checked_add(self, bytes: usize) -> Option<Self> {
        let new = Self(self.0.checked_add(bytes)?);

        let same_region = new.is_kernel_region() == self.is_kernel_region();
        let not_hole = new.is_kernel_region() || Self::userspace_range().contains(&new);

        // We shouldn't ever end up in the address space hole, nor should an
        // addition take us from userspace to kernelspace
        if !same_region || !not_hole {
            return None;
        }

        Some(new)
    }

    #[must_use]
    #[track_caller]
    pub fn offset(self, offset: isize) -> Self {
        match self.checked_offset(offset) {
            Some(address) => address,
            None => panic!("invalid virtual address: self={:#p}, offset={}", self, offset),
        }
    }

    pub fn checked_offset(self, offset: isize) -> Option<Self> {
        if offset.is_positive() {
            self.checked_add(offset as usize)
        } else {
            let offset = (-offset) as usize;
            let new = Self(self.0.checked_sub(offset)?);

            let same_region = new.is_kernel_region() == self.is_kernel_region();
            let not_hole = Self::kernelspace_range().contains(&new) || Self::userspace_range().contains(&new);

            // We shouldn't ever end up in the address space hole, nor should a
            // subtraction take us from kernelspace to userspace
            if !same_region || !not_hole {
                return None;
            }

            Some(new)
        }
    }

    /// # Safety
    ///
    /// You must ensure this cannot generate an invalid [`VirtualAddress`], e.g.
    /// that it does not lie in the address hole between user and kernel space
    #[must_use]
    pub unsafe fn unchecked_offset(self, offset: isize) -> Self {
        if offset.is_positive() {
            Self(self.0 + offset as usize)
        } else {
            let current = self.0;
            let offset = (-offset) as usize;

            Self(current - offset)
        }
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

    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub fn align_down_to(self, size: PageSize) -> Self {
        Self(self.0 & !(size.to_byte_size() - 1))
    }

    #[must_use]
    pub fn align_to_next(self, size: PageSize) -> Self {
        Self(self.align_down_to(size).0 + size.to_byte_size())
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

    pub fn is_aligned(self, page_size: PageSize) -> bool {
        self.offset_into_page(page_size) == 0
    }

    pub const fn from_vpns(vpns: [usize; N_VPN]) -> Self {
        #[cfg(feature = "paging.sv57")]
        compile_error!("sv57 stuff");

        let mut addr = 0;
        let mut shift = 12;

        // Replace this with a `for` once that's available in `const fn`s
        let mut i = 0;
        while i < N_VPN {
            addr |= vpns[i] << shift;
            shift += 9;

            i += 1;
        }

        let top_most_bit = 1 << (12 + N_VPN * 9 - 1);
        if addr & top_most_bit == top_most_bit {
            addr |= usize::MAX << (12 + N_VPN * 9 - 1);
        }

        VirtualAddress(addr)
    }

    pub fn is_kernel_region(self) -> bool {
        (self.0 as isize).is_negative()
    }

    pub const fn userspace_range() -> Range<VirtualAddress> {
        VirtualAddress::new(0)..VirtualAddress::new((1 << (12 + N_VPN * 9 - 1)) - 1)
    }

    pub const fn kernelspace_range() -> Range<VirtualAddress> {
        let mut vpns = [0; N_VPN];
        vpns[N_VPN - 1] = 256;

        // This should probably be a `..=` range, but...
        VirtualAddress::from_vpns(vpns)..VirtualAddress::new(usize::MAX)
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

    #[must_use]
    #[track_caller]
    pub fn offset(self, bytes: usize) -> Self {
        match self.0.checked_add(bytes) {
            Some(value) => Self(value),
            None => panic!("physical address wrapped overflow: {:#p} + {:#x}", self, bytes),
        }
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

    pub fn null() -> Self {
        Self(0)
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

        assert_eq!(addr % alignment_required, 0, "address alignment check failed");
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
