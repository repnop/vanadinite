pub mod heap;
pub mod paging;

use paging::Sv39PageTable;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct VirtualAddress(pub usize);

impl VirtualAddress {
    pub fn offset(self) -> usize {
        self.0 & 0xFFF
    }

    pub fn vpn(self) -> usize {
        self.0 & !0xFFF
    }

    fn vpn_i(self, i: usize) -> usize {
        let shift = i * 9 + 12;
        let mask = 0b1_1111_1111 << shift;
        (self.0 & mask) >> shift
    }

    pub fn to_physical_address(self, root_table: &Sv39PageTable) -> Option<PhysicalAddress> {
        const LEVELS: usize = 3;

        let mut table = root_table;

        for i in (0..LEVELS).rev() {
            let pte = table[self.vpn_i(i)].assert_valid();

            if pte.is_leaf() {
                // TODO: other physical address stuff (pg 71)
                return Some(PhysicalAddress(pte.ppn() | self.offset()));
            } else if i != 0 {
                table = unsafe { &*(pte.as_table_ptr()) };
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct PhysicalAddress(usize);
