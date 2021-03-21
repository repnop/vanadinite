use libvanadinite::{
    syscalls::{print::PrintErr, SyscallNumbers},
    KResult,
};

use crate::prelude::rust_2018::{AsRef, Sized};

#[inline(always)]
pub fn exit() -> ! {
    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            in("a0") SyscallNumbers::Exit as usize,
            options(noreturn),
        );
    }
}

#[inline]
pub fn print<T: AsRef<[u8]> + ?Sized>(value: &T) -> KResult<(), PrintErr> {
    let value = value.as_ref();
    let mut ret: core::mem::MaybeUninit<KResult<(), PrintErr>> = core::mem::MaybeUninit::uninit();

    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            in("a0") SyscallNumbers::Print as usize,
            in("a1") value.as_ptr(),
            in("a2") value.len(),
            in("a3") ret.as_mut_ptr(),
        );
    }

    unsafe { ret.assume_init() }
}

#[inline]
pub fn read_stdin(buffer: &mut [u8]) -> usize {
    let ret: usize;
    unsafe {
        #[rustfmt::skip]
        asm!(
            "ecall",
            inlateout("a0")  SyscallNumbers::ReadStdin as usize => ret,
            in("a1") buffer.as_ptr(),
            in("a2") buffer.len(),
        );
    }

    ret
}
