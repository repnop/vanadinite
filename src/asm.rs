mod inner {
    extern "C" {
        pub fn mhartid() -> usize;
        pub fn mvendorid() -> usize;
        pub fn misa() -> usize;
        pub fn ecall() -> !;
    }
}

#[inline(always)]
pub fn mhartid() -> usize {
    unsafe { inner::mhartid() }
}

#[inline(always)]
pub fn mvendorid() -> usize {
    unsafe { inner::mvendorid() }
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
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

#[inline(always)]
pub fn misa() -> Misa {
    Misa(unsafe { inner::misa() })
}

pub fn ecall() -> ! {
    unsafe { inner::ecall() }
}

// #[derive(Debug, Clone, Copy)]
// #[repr(C)]
// pub enum MCause {}
//
// pub fn mcause() -> MCause {}
