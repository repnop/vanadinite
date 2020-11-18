// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod isr;

use crate::sync::Mutex;

pub static PLIC: Mutex<Plic> = Mutex::new(Plic(None));

pub struct Plic(Option<&'static dyn crate::drivers::Plic>);

impl crate::drivers::Plic for Plic {
    fn enable_interrupt(&self, mode: crate::drivers::EnableMode, source: usize) {
        self.0.expect("no PLIC registered!").enable_interrupt(mode, source)
    }

    fn disable_interrupt(&self, mode: crate::drivers::EnableMode, source: usize) {
        self.0.expect("no PLIC registered!").disable_interrupt(mode, source)
    }

    fn interrupt_priority(&self, source: usize, priority: usize) {
        self.0.expect("no PLIC registered!").interrupt_priority(source, priority)
    }

    fn context_threshold(&self, mode: crate::drivers::EnableMode, threshold: usize) {
        self.0.expect("no PLIC registered!").context_threshold(mode, threshold)
    }

    fn is_pending(&self, source: usize) -> bool {
        self.0.expect("no PLIC registered!").is_pending(source)
    }

    fn claim(&self) -> Option<usize> {
        self.0.expect("no PLIC registered!").claim()
    }

    fn complete(&self, source: usize) {
        self.0.expect("no PLIC registered!").complete(source)
    }
}

unsafe impl Send for Plic {}
unsafe impl Sync for Plic {}

pub fn register_plic(plic: &'static dyn crate::drivers::Plic) {
    PLIC.lock().0 = Some(plic);
}

pub struct InterruptDisabler(());

impl InterruptDisabler {
    #[inline(always)]
    pub fn new() -> Self {
        crate::arch::csr::sstatus::disable_interrupts();
        Self(())
    }
}

impl Drop for InterruptDisabler {
    fn drop(&mut self) {
        crate::arch::csr::sstatus::enable_interrupts();
    }
}
