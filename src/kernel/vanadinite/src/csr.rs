// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod stvec {
    use core::arch::asm;
    #[inline(always)]
    pub fn set(ptr: unsafe extern "C" fn() -> !) {
        unsafe { asm!("csrw stvec, {}", in(reg) ptr) };
    }
}

pub mod sie {
    use core::arch::asm;
    #[inline(always)]
    pub fn enable() {
        unsafe { asm!("csrw sie, {}", in(reg) 0x222) };
    }

    #[inline(always)]
    pub fn read() -> usize {
        let val: usize;

        unsafe { asm!("csrr {}, sie", out(reg) val) };

        val
    }
}

pub mod sip {
    use core::arch::asm;
    #[inline(always)]
    pub fn read() -> usize {
        let val: usize;

        unsafe { asm!("csrr {}, sip", out(reg) val) };

        val
    }
}

pub mod sstatus {
    use core::arch::asm;
    pub fn enable_interrupts() {
        unsafe { asm!("csrsi sstatus, 2") };
    }

    pub fn disable_interrupts() {
        unsafe { asm!("csrci sstatus, 2") };
    }

    pub struct TemporaryUserMemoryAccess(bool);

    impl TemporaryUserMemoryAccess {
        pub fn new() -> Self {
            let disable_on_drop: usize;
            unsafe { asm!("csrr {}, sstatus", out(reg) disable_on_drop) };
            unsafe { asm!("csrs sstatus, {}", inout(reg) 1 << 18 => _) };

            Self((disable_on_drop >> 18) & 1 == 0)
        }
    }

    impl Drop for TemporaryUserMemoryAccess {
        fn drop(&mut self) {
            if self.0 {
                unsafe { asm!("csrc sstatus, {}", inout(reg) 1 << 18 => _) };
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(usize)]
    pub enum FloatingPointStatus {
        Off = 0,
        Initial = 1,
        Clean = 2,
        Dirty = 3,
    }

    pub fn fs() -> FloatingPointStatus {
        match (read() >> 13) & 3 {
            0 => FloatingPointStatus::Off,
            1 => FloatingPointStatus::Initial,
            2 => FloatingPointStatus::Clean,
            3 => FloatingPointStatus::Dirty,
            _ => unreachable!(),
        }
    }

    pub fn set_fs(status: FloatingPointStatus) {
        let val = (read() & !(3 << 13)) | ((status as usize) << 13);
        unsafe { asm!("csrw sstatus, {}", in(reg) val) };
    }

    #[inline(always)]
    pub fn read() -> usize {
        let val: usize;

        unsafe { asm!("csrr {}, sstatus", out(reg) val) };

        val
    }
}

pub mod time {
    use core::arch::asm;
    pub fn read() -> u64 {
        let value: u64;

        unsafe { asm!("csrr {}, time", out(reg) value) };

        value
    }
}

pub mod cycle {
    use core::arch::asm;
    pub fn read() -> usize {
        let value: usize;

        unsafe { asm!("csrr {}, cycle", out(reg) value) };

        value
    }
}

pub mod sscratch {
    use core::arch::asm;
    pub fn read() -> usize {
        let value: usize;

        unsafe { asm!("csrr {}, sscratch", out(reg) value) };

        value
    }

    pub fn write(value: usize) {
        unsafe { asm!("csrw sscratch, {}", in(reg) value) };
    }
}

pub mod satp {
    use crate::mem::paging::PhysicalAddress;
    use core::arch::asm;

    #[derive(Debug, Clone, Copy)]
    pub struct Satp {
        pub mode: SatpMode,
        pub asid: u16,
        pub root_page_table: PhysicalAddress,
    }

    impl Satp {
        pub fn as_usize(self) -> usize {
            ((self.mode as usize) << 60) | ((self.asid as usize) << 44) | self.root_page_table.ppn()
        }
    }

    #[inline(always)]
    pub fn read() -> Satp {
        let value: usize;
        unsafe { asm!("csrr {}, satp", out(reg) value) };

        let asid = ((value >> 44) & 0xFFFF) as u16;
        let root_page_table = PhysicalAddress::new(value << 12);
        let mode = match value >> 60 {
            0 => SatpMode::Bare,
            8 => SatpMode::Sv39,
            9 => SatpMode::Sv48,
            _ => unreachable!("invalid satp mode"),
        };

        Satp { mode, asid, root_page_table }
    }

    #[inline(always)]
    pub fn write(value: Satp) {
        let value = value.as_usize();
        unsafe { asm!("csrw satp, {}", in(reg) value) };
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(usize)]
    pub enum SatpMode {
        Bare = 0,
        Sv39 = 8,
        Sv48 = 9,
        Sv57 = 10,
        Sv64 = 11,
    }
}
