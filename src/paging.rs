#[repr(align(4096), C)]
pub struct Sv39PageTable {
    page_table_entries: [Sv39PageTableEntry; 512],
}

#[derive(Debug, Clone, Copy)]
pub struct Sv39PageTableEntry(u64);

impl Sv39PageTableEntry {
    #[inline(always)]
    const fn bit(self, n: usize) -> bool {
        self.0 & (1 << n) == (1 << n)
    }

    pub const fn valid(self) -> bool {
        self.bit(0)
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

    pub const fn rsw(self) -> u64 {
        (self.0 >> 8) & 0b11
    }

    pub const fn ppn0(self) -> u64 {
        (self.0 >> 10) & 0b1_1111_1111
    }

    pub const fn ppn1(self) -> u64 {
        (self.0 >> 19) & 0b1_1111_1111
    }

    pub const fn ppn2(self) -> u64 {
        (self.0 >> 28) & 0x3FF_FFFF
    }

    pub fn read_only(self) -> bool {
        self.readable() && !self.writable() && !self.executable()
    }

    pub fn read_write(self) -> bool {
        self.readable() && self.writable() && !self.executable()
    }

    pub fn execute_only(self) -> bool {
        !self.readable() && !self.writable() && self.executable()
    }

    pub fn read_execute(self) -> bool {
        self.readable() && !self.writable() && self.executable()
    }

    pub fn read_write_execute(self) -> bool {
        self.readable() && self.writable() && self.executable()
    }
}
