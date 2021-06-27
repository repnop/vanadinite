pub const fn plic_max_priority() -> usize {
    7
}

pub fn current_plic_context() -> usize {
    // first context is M-mode E51 monitor core which doesn't support S-mode so
    // we'll always be on hart >=1 which ends up working out to remove the +1
    // from the other fn
    plic_context_for(crate::HART_ID.get())
}

pub fn plic_context_for(hart: usize) -> usize {
    2 * hart
}
