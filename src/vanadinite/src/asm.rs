// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[inline(always)]
pub fn mhartid() -> usize {
    let hart_id;

    unsafe {
        asm!("csrr {}, mhartid", out(reg) hart_id);
    }

    hart_id
}

#[inline(always)]
pub fn mvendorid() -> usize {
    let vendor_id;

    unsafe {
        asm!("csrr {}, mhartid", out(reg) vendor_id);
    }

    vendor_id
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Misa(usize);

impl Misa {
    pub const fn mxl(self) -> usize {
        const USIZE_LEN_MIN_2: usize = core::mem::size_of::<usize>() - 2;
        32 * ((self.0 & (0b11 << USIZE_LEN_MIN_2)) >> USIZE_LEN_MIN_2)
    }

    pub const fn extensions(self) -> Extensions {
        Extensions(self.0 & 0x3FF_FFFF)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Extensions(usize);

impl core::fmt::Display for Extensions {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, ext) in EXTENSIONS.iter().enumerate() {
            if (self.0 >> i) & 1 == 1 {
                write!(f, "{}", ext)?;
            }
        }

        Ok(())
    }
}

pub const EXTENSIONS: [char; 26] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W',
    'X', 'Y', 'Z',
];

/// Returns the value of the MISA register which contains the following valid values:
///
/// 1: 32-bit
/// 2: 64-bit
/// 3: 128-bit
#[inline(always)]
pub fn misa() -> Misa {
    let misa;

    unsafe {
        asm!("csrr {}, misa", out(reg) misa);
    }

    Misa(misa)
}

#[inline(always)]
pub fn ecall() {
    unsafe {
        asm!("mv t2, zero");
        //asm!("fcvt.d.l t3, t2");
        asm!("fcvt.d.w f0, t2");
        asm!("li t0, 0xcafebabe");
        asm!("li t1, 0xdeadbeef");
        asm!("ecall");
    }
}

// #[derive(Debug, Clone, Copy)]
// #[repr(C)]
// pub enum MCause {}
//
// pub fn mcause() -> MCause {}

#[cfg(feature = "sifive_u")]
pub fn pause() {
    unsafe { asm!(".word 0x0100000F") };
}

#[cfg(not(feature = "sifive_u"))]
pub fn pause() {}
