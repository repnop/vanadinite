#[repr(transparent)]
pub struct LinkerSymbol(u8);

impl LinkerSymbol {
    pub fn as_ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }

    pub fn as_mut_ptr(&'static mut self) -> *mut u8 {
        self as *mut Self as *mut u8
    }

    pub fn as_usize(&'static self) -> usize {
        self.as_ptr() as usize
    }
}

unsafe impl Sync for LinkerSymbol {}
unsafe impl Send for LinkerSymbol {}

#[inline(always)]
pub fn manual_debug_point() {
    unsafe {
        asm!("1: j 1b");
    }
}
