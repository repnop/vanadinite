// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    csr::satp,
    mem::{
        paging::ToPermissions,
        phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
        phys2virt,
    },
    utils::Units,
};

pub const PHYS_PPN_MASK: usize = 0x0FFF_FFFF_FFFF;

#[repr(C, align(4096))]
pub struct Sv39PageTable {
    pub entries: [PageTableEntry; 512],
}

impl Sv39PageTable {
    pub const fn new() -> Self {
        Self { entries: [PageTableEntry::new(); 512] }
    }

    #[track_caller]
    pub fn map<P>(&mut self, phys: PhysicalAddress, virt: VirtualAddress, size: PageSize, perms: P)
    where
        P: ToPermissions,
    {
        PageSize::assert_addr_aligned(size, phys.as_usize());
        PageSize::assert_addr_aligned(size, virt.as_usize());

        let [kib_index, mib_index, gib_index] = virt.vpns();

        let pte = match size {
            PageSize::Gigapage => &mut self.entries[gib_index],
            PageSize::Megapage => {
                let next = Self::get_or_alloc_next_level(&mut self.entries[gib_index]);
                &mut next.entries[mib_index]
            }
            PageSize::Kilopage => {
                let next = Self::get_or_alloc_next_level(&mut self.entries[gib_index]);
                let next = Self::get_or_alloc_next_level(&mut next.entries[mib_index]);
                &mut next.entries[kib_index]
            }
        };

        assert!(!pte.valid(), "Page table entry already populated mapping {:#p} -> {:#p}!", phys, virt);
        pte.make_leaf(phys, perms);
    }

    #[track_caller]
    pub fn unmap(&mut self, virt: VirtualAddress) -> Option<(PhysicalAddress, PageSize)> {
        let (entry, _size) = self.entry_mut(virt).expect("Attempted to unmap an already unmapped page!");
        *entry = PageTableEntry::new();

        None

        // FIXME: Free page table if able
    }

    pub fn is_mapped(&self, virt: VirtualAddress) -> bool {
        self.translate(virt).is_some()
    }

    pub fn translate(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        self.entry(virt).and_then(|(entry, size)| Some(entry.ppn()?.offset(virt.offset_into_page(size))))
    }

    pub fn entry(&self, virt: VirtualAddress) -> Option<(&PageTableEntry, PageSize)> {
        let [kib_index, mib_index, gib_index] = virt.vpns();

        let gib_entry = &self.entries[gib_index];
        let next_table = match gib_entry.kind() {
            EntryKind::Leaf => return Some((gib_entry, PageSize::Gigapage)),
            EntryKind::NotValid => return None,
            EntryKind::Branch(phys) => unsafe { &*phys2virt(phys).as_ptr().cast::<Sv39PageTable>() },
        };

        let mib_entry = &next_table.entries[mib_index];
        let next_table = match mib_entry.kind() {
            EntryKind::Leaf => return Some((mib_entry, PageSize::Megapage)),
            EntryKind::NotValid => return None,
            EntryKind::Branch(phys) => unsafe { &*phys2virt(phys).as_ptr().cast::<Sv39PageTable>() },
        };

        let kib_entry = &next_table.entries[kib_index];
        match kib_entry.kind() {
            EntryKind::Leaf => Some((kib_entry, PageSize::Kilopage)),
            EntryKind::NotValid => None,
            EntryKind::Branch(_) => unreachable!("A KiB PTE was marked as a branch?"),
        }
    }

    pub fn entry_mut(&mut self, virt: VirtualAddress) -> Option<(&mut PageTableEntry, PageSize)> {
        let [kib_index, mib_index, gib_index] = virt.vpns();

        let gib_entry = &mut self.entries[gib_index];
        let next_table = match gib_entry.kind() {
            EntryKind::Leaf => return Some((gib_entry, PageSize::Gigapage)),
            EntryKind::NotValid => return None,
            EntryKind::Branch(phys) => unsafe { &mut *phys2virt(phys).as_mut_ptr().cast::<Sv39PageTable>() },
        };

        let mib_entry = &mut next_table.entries[mib_index];
        let next_table = match mib_entry.kind() {
            EntryKind::Leaf => return Some((mib_entry, PageSize::Megapage)),
            EntryKind::NotValid => return None,
            EntryKind::Branch(phys) => unsafe { &mut *phys2virt(phys).as_mut_ptr().cast::<Sv39PageTable>() },
        };

        let kib_entry = &mut next_table.entries[kib_index];
        match kib_entry.kind() {
            EntryKind::Leaf => Some((kib_entry, PageSize::Kilopage)),
            EntryKind::NotValid => None,
            EntryKind::Branch(_) => unreachable!("A KiB PTE was marked as a branch?"),
        }
    }

    /// # Safety
    ///
    /// This function assumes that `satp` holds a valid physical pointer to a
    /// page table that can be safely converted with [`phys2virt`](crate::mem::phys2virt)
    #[inline(always)]
    pub unsafe fn current() -> *mut Sv39PageTable {
        phys2virt(satp::read().root_page_table).as_mut_ptr().cast()
    }

    #[track_caller]
    fn get_or_alloc_next_level(entry: &mut PageTableEntry) -> &mut Sv39PageTable {
        match entry.kind() {
            EntryKind::Branch(phys) => unsafe { &mut *phys2virt(phys).as_mut_ptr().cast() },
            EntryKind::NotValid => unsafe {
                let page = PHYSICAL_MEMORY_ALLOCATOR.lock().alloc().expect("out of memory!");
                // make sure the memory is initialized before we convert to
                // a exclusive reference
                let ptr = phys2virt(page.as_phys_address()).as_mut_ptr().cast::<Sv39PageTable>();
                ptr.write(Sv39PageTable::new());

                entry.make_branch(page.as_phys_address());

                &mut *ptr
            },
            EntryKind::Leaf => panic!("Attempting to overwrite a leaf entry somehow"),
        }
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
        let permissions = (permissions.into_permissions() as usize) << 1;
        self.0 = (phys.ppn() << 10) | permissions | 1;
    }

    pub fn set_permissions(&mut self, permissions: impl ToPermissions) {
        let permissions = (permissions.into_permissions() as usize) << 1;
        self.0 = (self.0 & !(0b11110)) | permissions;
    }

    pub fn is_readable(self) -> bool {
        self.0 & 2 == 2
    }

    pub fn is_writable(self) -> bool {
        self.0 & 4 == 4
    }

    pub fn make_branch(&mut self, next_level: PhysicalAddress) {
        let ppn = next_level.ppn();
        self.0 = (ppn << 10) | 1;
    }

    pub fn kind(self) -> EntryKind {
        match (self.valid(), self.is_branch()) {
            (true, true) => EntryKind::Branch(PhysicalAddress::new(((self.0 >> 10) & 0x0FFF_FFFF_FFFF) << 12)),
            (true, false) => EntryKind::Leaf,
            (false, _) => EntryKind::NotValid,
        }
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

    pub fn offset_into_page(self, page_size: PageSize) -> usize {
        match page_size {
            PageSize::Gigapage => self.0 & 0x3FFFFFFF,
            PageSize::Megapage => self.0 & 0x1FFFFF,
            PageSize::Kilopage => self.0 & 0xFFF,
        }
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
        let alignment_required = self.to_byte_size();

        assert_eq!(addr % alignment_required, 0, "physical address alignment check failed");
    }

    pub fn to_byte_size(self) -> usize {
        match self {
            PageSize::Kilopage => 4.kib(),
            PageSize::Megapage => 2.mib(),
            PageSize::Gigapage => 1.gib(),
        }
    }
}
