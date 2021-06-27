pub const fn plic_max_priority() -> usize {
    31
}

pub fn current_plic_context() -> usize {
    plic_context_for(crate::HART_ID.get())
}

pub fn plic_context_for(hart: usize) -> usize {
    1 + 2 * hart
}
