// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod stvec {
    #[inline(always)]
    pub fn set(ptr: extern "C" fn()) {
        unsafe { asm!("csrw stvec, {}", in(reg) ptr) };
    }
}

pub mod sie {
    #[inline(always)]
    pub fn enable() {
        unsafe {
            asm!(
                "li {tmp}, 0x222",
                "csrw sie, {tmp}",
                tmp = out(reg) _,
            );
        }
    }

    #[inline(always)]
    pub fn read() -> usize {
        let val: usize;

        unsafe { asm!("csrr {}, sie", out(reg) val) };

        val
    }
}

pub mod sip {
    #[inline(always)]
    pub fn read() -> usize {
        let val: usize;

        unsafe { asm!("csrr {}, sip", out(reg) val) };

        val
    }
}

pub mod sstatus {
    pub fn enable_interrupts() {
        unsafe {
            asm!(
                "csrr {0}, sstatus",
                "ori {0}, {0}, 2",
                "csrw sstatus, {0}",
                out(reg) _,
            );
        }
    }

    pub fn disable_interrupts() {
        unsafe {
            asm!(
                "csrr {0}, sstatus",
                "li {1}, 2",
                "not {1}, {1}",
                "and {0}, {0}, {1}",
                "csrw sstatus, {0}",
                out(reg) _,
                out(reg) _,
            );
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
    pub fn read() -> usize {
        let value: usize;

        unsafe { asm!("csrr {}, time", out(reg) value) };

        value
    }
}

pub mod cycle {
    pub fn read() -> usize {
        let value: usize;

        unsafe { asm!("csrr {}, cycle", out(reg) value) };

        value
    }
}

pub mod sscratch {
    pub fn read() -> usize {
        let value: usize;

        unsafe { asm!("csrr {}, sscratch", out(reg) value) };

        value
    }

    pub fn write(value: usize) {
        unsafe { asm!("csrw sscratch, {}", in(reg) value) };
    }
}
