use core::marker::PhantomData;

#[repr(align(4096), C)]
pub struct Sv39PageTable {
    page_table_entries: [Sv39PageTableEntry<Unvalidated>; 512],
}

impl Sv39PageTable {
    pub fn new() -> Self {
        Self {
            page_table_entries: [Sv39PageTableEntry::default(); 512],
        }
    }
}

impl core::ops::Index<usize> for Sv39PageTable {
    type Output = Sv39PageTableEntry<Unvalidated>;

    fn index(&self, idx: usize) -> &Sv39PageTableEntry<Unvalidated> {
        &self.page_table_entries[idx]
    }
}

impl core::ops::IndexMut<usize> for Sv39PageTable {
    fn index_mut(&mut self, idx: usize) -> &mut Sv39PageTableEntry<Unvalidated> {
        &mut self.page_table_entries[idx]
    }
}

#[derive(Debug)]
pub enum Unvalidated {}
#[derive(Debug)]
pub enum Validated {}

#[derive(Debug)]
#[repr(transparent)]
pub struct Sv39PageTableEntry<T>(usize, PhantomData<T>);

impl<T> Sv39PageTableEntry<T> {
    pub fn unvalidate(self) -> Sv39PageTableEntry<Unvalidated> {
        Sv39PageTableEntry(self.0, PhantomData)
    }
}

impl Clone for Sv39PageTableEntry<Unvalidated> {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Sv39PageTableEntry<Unvalidated> {}

impl Default for Sv39PageTableEntry<Unvalidated> {
    fn default() -> Self {
        Self(0, PhantomData)
    }
}

impl Sv39PageTableEntry<Unvalidated> {
    pub fn as_ptr(&self) -> *const Self {
        self as *const Self
    }

    pub fn as_mut(&mut self) -> *mut Self {
        self as *mut Self
    }

    pub fn validate(self) -> Option<Sv39PageTableEntry<Validated>> {
        if self.0 & 1 == 1 {
            Some(Sv39PageTableEntry(self.0, PhantomData))
        } else {
            None
        }
    }

    pub fn assert_valid(self) -> Sv39PageTableEntry<Validated> {
        assert!(
            self.0 & 1 == 1,
            "assert: attempted to use invalid page as valid"
        );
        Sv39PageTableEntry(self.0, PhantomData)
    }

    pub fn validate_ref(&self) -> Option<&Sv39PageTableEntry<Validated>> {
        if self.0 & 1 == 1 {
            Some(unsafe { &*(self.as_ptr().cast()) })
        } else {
            None
        }
    }

    pub fn assert_valid_ref(&self) -> &Sv39PageTableEntry<Validated> {
        assert!(
            self.0 & 1 == 1,
            "assert: attempted to use invalid page as valid"
        );
        unsafe { &*(self.as_ptr().cast()) }
    }

    pub fn validate_mut(&mut self) -> Option<&mut Sv39PageTableEntry<Validated>> {
        if self.0 & 1 == 1 {
            Some(unsafe { &mut *(self.as_mut().cast()) })
        } else {
            None
        }
    }

    pub fn assert_valid_mut(&mut self) -> &mut Sv39PageTableEntry<Validated> {
        assert!(
            self.0 & 1 == 1,
            "assert: attempted to use invalid page as valid"
        );
        unsafe { &mut *(self.as_mut().cast()) }
    }

    pub fn validate_or_else<F: FnOnce() -> Sv39PageTableEntry<Validated>>(
        &mut self,
        f: F,
    ) -> &mut Sv39PageTableEntry<Validated> {
        let valid = self.0 & 1 == 1;

        if !valid {
            *self = f().unvalidate();
        }

        unsafe { &mut *(self.as_mut().cast()) }
    }
}

impl Clone for Sv39PageTableEntry<Validated> {
    fn clone(&self) -> Sv39PageTableEntry<Validated> {
        Self(self.0, PhantomData)
    }
}

impl Sv39PageTableEntry<Validated> {
    pub fn new() -> Self {
        Self(1, PhantomData)
    }

    #[inline(always)]
    const fn bit(self, n: usize) -> bool {
        self.0 & (1 << n) == (1 << n)
    }

    #[inline(always)]
    fn set_bit(&mut self, n: usize) {
        self.0 |= 1 << n;
    }

    #[inline(always)]
    fn clear_bit(&mut self, n: usize) {
        self.0 &= !(1 << n)
    }

    #[inline(always)]
    fn assign_bit(&mut self, n: usize, b: bool) {
        #[allow(clippy::match_bool)]
        match b {
            true => self.set_bit(n),
            false => self.clear_bit(n),
        }
    }

    pub const fn readable(self) -> bool {
        self.bit(1)
    }

    pub const fn writable(self) -> bool {
        self.bit(2)
    }

    pub const fn executable(self) -> bool {
        self.bit(3)
    }

    pub const fn user(self) -> bool {
        self.bit(4)
    }

    pub const fn global(self) -> bool {
        self.bit(5)
    }

    pub const fn accessed(self) -> bool {
        self.bit(6)
    }

    pub const fn dirty(self) -> bool {
        self.bit(7)
    }

    pub const fn rsw(self) -> usize {
        (self.0 >> 8) & 0b11
    }

    pub fn ppn(self) -> usize {
        self.0 & (0xFFF_FFFF_FFFF << 10)
    }

    pub fn set_next_page_table(&mut self, next_table: &Sv39PageTable) {
        self.0 &= !(0xFFF_FFFF_FFFF << 10);
        self.0 |= (next_table as *const _ as usize) & (0xFFF_FFFF_FFFF << 10);
    }

    pub fn set_ppn(&mut self, ppn: *const u8) {
        assert!(
            ppn as usize % 4096 == 0,
            "assert: unaligned physical page pointer: {:#p}",
            ppn
        );
        self.0 &= !(0xFFF_FFFF_FFFF << 10);
        self.0 |= (ppn as usize) & (0xFFF_FFFF_FFFF << 10);
    }

    pub fn to_table_ptr(self) -> *const Sv39PageTable {
        self.ppn() as *const Sv39PageTable
    }

    pub fn to_table_mut_ptr(self) -> *mut Sv39PageTable {
        self.ppn() as *mut Sv39PageTable
    }

    pub unsafe fn to_table_ref(&self) -> &Sv39PageTable {
        self.clone().to_table_ptr().as_ref().unwrap()
    }

    pub unsafe fn to_table_mut(&mut self) -> &mut Sv39PageTable {
        self.clone().to_table_mut_ptr().as_mut().unwrap()
    }

    //pub fn read_only(self) -> bool {
    //    self.readable() && !self.writable() && !self.executable()
    //}
    //
    //pub fn read_write(self) -> bool {
    //    self.readable() && self.writable() && !self.executable()
    //}
    //
    //pub fn execute_only(self) -> bool {
    //    !self.readable() && !self.writable() && self.executable()
    //}
    //
    //pub fn read_execute(self) -> bool {
    //    self.readable() && !self.writable() && self.executable()
    //}
    //
    //pub fn read_write_execute(self) -> bool {
    //    self.readable() && self.writable() && self.executable()
    //}

    pub fn permissions(&self) -> Permissions {
        let perm = self.0 & 0b1110;
        assert_ne!(perm, 0b100, "assert: write-only page");
        unsafe { core::mem::transmute(perm) }
    }

    pub fn set_permissions(&mut self, permissions: Permissions) {
        self.0 &= !(0b1110);
        self.0 |= permissions as usize;
    }

    pub fn is_leaf(&self) -> bool {
        if let Permissions::None = self.permissions() {
            false
        } else {
            true
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Permissions {
    None = 0b0000,
    Read = 0b10,
    Execute = 0b1000,
    ReadWrite = 0b110,
    ReadExecute = 0b1010,
    ReadWriteExecute = 0b1110,
}

impl core::ops::BitOr for Permissions {
    type Output = Permissions;

    fn bitor(self, rhs: Self) -> Self {
        use Permissions::*;

        match (self, rhs) {
            (None, _) => rhs,
            (_, None) => self,
            (Execute, Execute) => Execute,
            (Execute, Read) => ReadExecute,
            (Execute, ReadExecute) => ReadExecute,
            (Execute, ReadWrite) => ReadWriteExecute,
            (Execute, ReadWriteExecute) => ReadWriteExecute,
            (Read, Execute) => ReadExecute,
            (Read, _) => rhs,
            (_, Read) => self,
            (ReadExecute, Execute) => ReadExecute,
            (ReadExecute, ReadExecute) => ReadExecute,
            (ReadExecute, ReadWrite) => ReadWriteExecute,
            (ReadExecute, ReadWriteExecute) => ReadWriteExecute,
            (ReadWrite, Execute) => ReadWriteExecute,
            (ReadWrite, ReadExecute) => ReadWriteExecute,
            (ReadWrite, ReadWrite) => ReadWrite,
            (ReadWrite, ReadWriteExecute) => ReadWriteExecute,
            (ReadWriteExecute, Execute) => ReadWriteExecute,
            (ReadWriteExecute, ReadExecute) => ReadWriteExecute,
            (ReadWriteExecute, ReadWrite) => ReadWriteExecute,
            (ReadWriteExecute, ReadWriteExecute) => ReadWriteExecute,
        }
    }
}
