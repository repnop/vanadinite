// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub type InterruptServiceRoutine = fn(usize) -> Result<(), &'static str>;

const ISR_LIMIT: usize = 128;

pub(super) static ISR_REGISTRY: [IsrEntry; ISR_LIMIT] = [IsrEntry::default(); ISR_LIMIT];

#[derive(Debug)]
pub struct IsrEntry {
    active: AtomicBool,
    f: AtomicUsize,
    a0: AtomicUsize,
}

impl IsrEntry {
    const fn default() -> Self {
        Self { active: AtomicBool::new(false), f: AtomicUsize::new(0), a0: AtomicUsize::new(0) }
    }

    pub fn new(isr: InterruptServiceRoutine, a0: usize) -> Self {
        Self { active: AtomicBool::new(true), f: AtomicUsize::new(isr as usize), a0: AtomicUsize::new(a0) }
    }
}

pub fn register_isr(interrupt_id: usize, entry: IsrEntry) {
    let slot = &ISR_REGISTRY[interrupt_id];

    slot.active.store(entry.active.load(Ordering::Relaxed), Ordering::SeqCst);
    slot.f.store(entry.f.load(Ordering::Relaxed), Ordering::SeqCst);
    slot.a0.store(entry.a0.load(Ordering::Relaxed), Ordering::SeqCst);
}
