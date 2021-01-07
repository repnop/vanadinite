#[inline(always)]
pub fn exit() -> ! {
    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            in("a0") 0,
            options(noreturn),
        );
    }
}

#[inline(always)]
pub fn print<T: crate::prelude::v1::AsRef<[u8]> + ?crate::prelude::v1::Sized>(value: &T) {
    let value = value.as_ref();
    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            in("a0") 1,
            in("a1") value.as_ptr(),
            in("a2") value.len(),
        );
    }
}

#[inline(always)]
pub fn read_stdin(buffer: &mut [u8]) -> usize {
    let ret: usize;
    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            inlateout("a0") 2usize => ret,
            in("a1") buffer.as_ptr(),
            in("a2") buffer.len(),
        );
    }

    ret
}
