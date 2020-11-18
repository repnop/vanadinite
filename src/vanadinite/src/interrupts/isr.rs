// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::drivers::InterruptServicable;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub type IsrCallback = fn(usize, usize) -> Result<(), &'static str>;

const ISR_LIMIT: usize = 128;

pub(super) static ISR_REGISTRY: [IsrEntry; ISR_LIMIT] = [IsrEntry::new(); ISR_LIMIT];

#[derive(Debug)]
pub struct IsrEntry {
    active: AtomicBool,
    f: AtomicUsize,
    private: AtomicUsize,
}

impl IsrEntry {
    const fn new() -> Self {
        Self { active: AtomicBool::new(false), f: AtomicUsize::new(0), private: AtomicUsize::new(0) }
    }
}

pub fn register_isr<T: InterruptServicable>(interrupt_id: usize, private: usize) {
    log::info!("Registering ISR for {} on ID {}", core::any::type_name::<T>(), interrupt_id);
    let _disabler = super::InterruptDisabler::new();
    let slot = &ISR_REGISTRY[interrupt_id];

    slot.active.store(true, Ordering::SeqCst);
    slot.f.store(<T as InterruptServicable>::isr as usize, Ordering::SeqCst);
    slot.private.store(private, Ordering::SeqCst);
}

pub fn isr_entry(id: usize) -> Option<(IsrCallback, usize)> {
    let entry = &ISR_REGISTRY[id];

    if entry.active.load(Ordering::Relaxed) {
        Some((unsafe { core::mem::transmute(entry.f.load(Ordering::Relaxed)) }, entry.private.load(Ordering::Relaxed)))
    } else {
        None
    }
}
