pub mod stvec {
    #[inline(always)]
    pub fn set(ptr: extern "C" fn()) {
        unsafe { asm!("csrw stvec, {}", in(reg) ptr) };
    }
}
