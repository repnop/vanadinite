pub trait PtrUtils {
    type Output;

    fn assert_aligned(self, align: usize);
    fn assert_aligned_to<U>(self);
    fn assert_aligned_to_self(self);
    unsafe fn align_up(self, align: usize) -> Self;
    unsafe fn align_up_to<U>(self) -> Self;
    unsafe fn align_up_to_self(self) -> Self;
    unsafe fn read_and_increment(&mut self) -> Self::Output;
}

impl<T> PtrUtils for *const T {
    type Output = T;

    fn assert_aligned(self, align: usize) {
        assert!(align.is_power_of_two());
        assert_eq!(self as usize % align, 0, "assert: unaligned ptr");
    }

    fn assert_aligned_to<U>(self) {
        self.assert_aligned(core::mem::align_of::<U>());
    }

    fn assert_aligned_to_self(self) {
        self.assert_aligned(core::mem::align_of::<T>());
    }

    unsafe fn align_up(self, align: usize) -> Self {
        let offset = self.align_offset(align);
        assert_ne!(offset, usize::max_value(), "assert: couldn't align pointer");

        self.add(offset)
    }

    unsafe fn align_up_to<U>(self) -> Self {
        self.align_up(core::mem::align_of::<U>())
    }

    unsafe fn align_up_to_self(self) -> Self {
        self.align_up(core::mem::align_of::<T>())
    }

    unsafe fn read_and_increment(&mut self) -> Self::Output {
        let t = self.read();
        *self = self.add(1);

        t
    }
}

impl<T> PtrUtils for *mut T {
    type Output = T;

    fn assert_aligned(self, align: usize) {
        assert!(align.is_power_of_two());
        assert_eq!(self as usize % align, 0, "assert: unaligned ptr");
    }

    fn assert_aligned_to<U>(self) {
        self.assert_aligned(core::mem::align_of::<U>());
    }

    fn assert_aligned_to_self(self) {
        self.assert_aligned(core::mem::align_of::<T>());
    }

    unsafe fn align_up(self, align: usize) -> Self {
        let offset = self.align_offset(align);
        assert_ne!(offset, usize::max_value(), "assert: couldn't align pointer");

        self.add(offset)
    }

    unsafe fn align_up_to<U>(self) -> Self {
        self.align_up(core::mem::align_of::<U>())
    }

    unsafe fn align_up_to_self(self) -> Self {
        self.align_up(core::mem::align_of::<T>())
    }

    unsafe fn read_and_increment(&mut self) -> Self::Output {
        let t = self.read();
        *self = self.add(1);

        t
    }
}

#[repr(transparent)]
pub struct LinkerSymbol(core::cell::UnsafeCell<u8>);

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
