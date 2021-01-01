// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::perms::ToPermissions;

#[repr(C, align(4096))]
pub struct Sv39PageTable {
    pub entries: [PageTableEntry; 512],
}

impl Sv39PageTable {
    pub const fn new() -> Self {
        Self { entries: [PageTableEntry::new(); 512] }
    }

    #[track_caller]
    pub fn map<F, A, P>(
        &mut self,
        phys: PhysicalAddress,
        virt: VirtualAddress,
        size: PageSize,
        permissions: P,
        mut page_alloc: F,
        address_conversion: A,
    ) where
        F: FnMut() -> (*mut Sv39PageTable, PhysicalAddress),
        A: Fn(PhysicalAddress) -> VirtualAddress,
        P: ToPermissions,
    {
        size.assert_addr_aligned(phys.0);
        size.assert_addr_aligned(virt.0);

        let mut page_table = self;
        let mut pte = &mut page_table.entries[virt.vpns()[2]];

        for i in size.i() {
            if pte.is_branch() {
                let addr = pte.subtable().unwrap();
                page_table = unsafe { &mut *address_conversion(addr).as_mut_ptr().cast() };
            } else {
                let (pt, phys_addr) = page_alloc();
                pte.make_branch(phys_addr);
                page_table = unsafe { &mut *pt };
            }
            pte = &mut page_table.entries[virt.vpns()[i]];
        }

        assert!(!pte.valid(), "Sv39PageTable::map: {:#p} -> {:#p}, page table entry already populated!", phys, virt);

        pte.make_leaf(phys, permissions);
    }

    /// # Safety
    /// This method ***MUST*** be called with the exact inverse function with
    /// which created the initial page table mappings
    ///
    /// Unmaps a page table and returns the virtual address of the last level
    /// table if the mapping exists
    pub unsafe fn unmap<F>(&mut self, virt: VirtualAddress, f: F) -> Option<VirtualAddress>
    where
        F: Fn(PhysicalAddress) -> VirtualAddress,
    {
        let mut va = None;
        let mut pt = self;

        for vpn in virt.vpns().iter().copied().rev() {
            let entry = &mut pt.entries[vpn];
            if entry.is_leaf() {
                *entry = PageTableEntry::new();
                break;
            }

            let pt_virt = f(entry.subtable().unwrap());

            va = Some(pt_virt);
            pt = &mut *(pt_virt.as_mut_ptr() as *mut Sv39PageTable);
        }

        va
    }

    pub fn is_mapped<F>(&self, virt: VirtualAddress, address_conversion: F) -> bool
    where
        F: Fn(PhysicalAddress) -> VirtualAddress,
    {
        self.translate(virt, address_conversion).is_some()
    }

    pub fn translate<F>(&self, virt: VirtualAddress, address_conversion: F) -> Option<PhysicalAddress>
    where
        F: Fn(PhysicalAddress) -> VirtualAddress,
    {
        let mut page_table = self;
        let mut page_size = PageSize::Gigapage;

        for vpn in virt.vpns().iter().copied().rev() {
            let pte = &page_table.entries[vpn];
            if pte.is_branch() {
                page_table = unsafe { &*address_conversion(pte.subtable().unwrap()).as_ptr().cast() };

                page_size = match page_size {
                    PageSize::Gigapage => PageSize::Megapage,
                    _ => PageSize::Kilopage,
                };
            } else if pte.valid() {
                return pte.ppn().map(|p| p.offset(virt.offset_into_page(page_size)));
            }
        }

        None
    }

    pub fn entry<F>(&self, virt: VirtualAddress, address_conversion: F) -> Option<&PageTableEntry>
    where
        F: Fn(PhysicalAddress) -> VirtualAddress,
    {
        let mut page_table = self;
        let mut page_size = PageSize::Gigapage;

        for vpn in virt.vpns().iter().copied().rev() {
            let pte = &page_table.entries[vpn];
            if pte.is_branch() {
                page_table = unsafe { &*address_conversion(pte.subtable().unwrap()).as_ptr().cast() };

                page_size = match page_size {
                    PageSize::Gigapage => PageSize::Megapage,
                    _ => PageSize::Kilopage,
                };
            } else if pte.valid() {
                return Some(pte);
            }
        }

        None
    }

    pub fn entry_mut<F>(&mut self, virt: VirtualAddress, address_conversion: F) -> Option<&mut PageTableEntry>
    where
        F: Fn(PhysicalAddress) -> VirtualAddress,
    {
        let mut page_table = self;
        let mut page_size = PageSize::Gigapage;

        for vpn in virt.vpns().iter().copied().rev() {
            let pte = unsafe { &mut *(&raw mut page_table.entries[vpn]) };
            if pte.is_branch() {
                page_table = unsafe { &mut *address_conversion(pte.subtable().unwrap()).as_mut_ptr().cast() };

                page_size = match page_size {
                    PageSize::Gigapage => PageSize::Megapage,
                    _ => PageSize::Kilopage,
                };
            } else if pte.valid() {
                return Some(pte);
            }
        }

        None
    }

    pub unsafe fn current() -> *mut Sv39PageTable {
        let satp: usize;
        asm!("csrr {}, satp", out(reg) satp);

        crate::mem::phys2virt(PhysicalAddress::new((satp & 0x0FFF_FFFF_FFFF) << 12)).as_mut_ptr().cast()
    }
}

impl Default for Sv39PageTable {
    fn default() -> Self {
        Self { entries: [PageTableEntry::default(); 512] }
    }
}

impl core::fmt::Debug for Sv39PageTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Sv39PageTable {{ ... }}")
    }
}

#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(usize);

// TODO: ensure upper bits are zeroed?
impl PageTableEntry {
    pub const fn new() -> Self {
        PageTableEntry(0)
    }

    pub fn valid(self) -> bool {
        self.0 & 1 == 1
    }

    pub fn make_leaf(&mut self, phys: PhysicalAddress, permissions: impl ToPermissions) {
        let permissions = (permissions.to_permissions() as usize) << 1;
        self.0 = (phys.ppn() << 10) | permissions | 1;
    }

    pub fn set_permissions(&mut self, permissions: impl ToPermissions) {
        let permissions = (permissions.to_permissions() as usize) << 1;
        self.0 = (self.0 & !(0b11110)) | permissions;
    }

    pub fn is_readable(self) -> bool {
        self.0 & 2 == 2
    }

    pub fn make_branch(&mut self, next_level: PhysicalAddress) {
        let ppn = next_level.ppn();
        self.0 = (ppn << 10) | 1;
    }

    pub fn is_leaf(self) -> bool {
        self.valid() && (self.0 & 0b1110 != 0)
    }

    pub fn is_branch(self) -> bool {
        self.valid() && (self.0 & 0b1110 == 0)
    }

    pub fn subtable(self) -> Option<PhysicalAddress> {
        match self.is_branch() {
            true => Some(PhysicalAddress::new((self.0 >> 10) << 12)),
            false => None,
        }
    }

    pub fn ppn(self) -> Option<PhysicalAddress> {
        match self.valid() {
            true => Some(PhysicalAddress::new(((self.0 >> 10) & 0x0FFF_FFFF_FFFF) << 12)),
            false => None,
        }
    }
}

impl core::fmt::Pointer for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&(self.0 as *const u8), f)
    }
}

#[derive(Debug, Clone, Copy)]
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

    pub fn vpns(self) -> [usize; 3] {
        const VPN_BITMASK: usize = 0x1FF;

        [(self.0 >> 12) & VPN_BITMASK, (self.0 >> 21) & VPN_BITMASK, (self.0 >> 30) & VPN_BITMASK]
    }

    pub fn offset_into_page(self, page_size: PageSize) -> usize {
        match page_size {
            PageSize::Gigapage => self.0 & 0x3FFFFFFF,
            PageSize::Megapage => self.0 & 0x1FFFFF,
            PageSize::Kilopage => self.0 & 0xFFF,
        }
    }

    pub fn is_kernel_region(self) -> bool {
        (self.0 as isize).is_negative()
    }
}

impl core::fmt::Pointer for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&(self.0 as *const u8), f)
    }
}

#[derive(Debug, Clone, Copy)]
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
        (self.0 >> 12) & 0x0FFF_FFFF_FFFF
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum PageSize {
    Kilopage,
    Megapage,
    Gigapage,
}

impl PageSize {
    #[track_caller]
    fn assert_addr_aligned(self, addr: usize) {
        let alignment_required = match self {
            PageSize::Kilopage => 4 * 1024,
            PageSize::Megapage => 2 * 1024 * 1024,
            PageSize::Gigapage => 1 * 1024 * 1024 * 1024,
        };

        assert_eq!(addr % alignment_required, 0, "physical address alignment check failed");
    }

    fn i(self) -> impl Iterator<Item = usize> {
        match self {
            PageSize::Kilopage => (0..2).rev(),
            PageSize::Megapage => (1..2).rev(),
            PageSize::Gigapage => (2..2).rev(),
        }
    }
}
