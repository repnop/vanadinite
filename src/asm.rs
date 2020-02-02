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

    pub const fn extensions(self) -> usize {
        self.0 & 0x3FF_FFFF
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
