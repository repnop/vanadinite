#[repr(transparent)]
pub struct LinkerSymbol(u8);

impl LinkerSymbol {
    pub fn as_ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }

    pub fn as_mut_ptr(&'static mut self) -> *mut u8 {
        self as *mut Self as *mut u8
    }
}

unsafe impl Sync for LinkerSymbol {}
unsafe impl Send for LinkerSymbol {}
