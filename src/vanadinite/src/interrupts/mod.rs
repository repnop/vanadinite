// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod isr;

use crate::{drivers::generic::plic, sync::Mutex};

pub static PLIC: Mutex<Option<&'static plic::Plic>> = Mutex::new(None);

pub fn register_plic(plic: &'static plic::Plic) {
    *PLIC.lock() = Some(plic);
}

pub struct InterruptDisabler(bool);

impl InterruptDisabler {
    #[allow(clippy::clippy::new_without_default)]
    #[inline(always)]
    pub fn new() -> Self {
        let reenable = match crate::csr::sstatus::read() & 2 == 2 {
            true => {
                crate::csr::sstatus::disable_interrupts();
                true
            }
            false => false,
        };

        Self(reenable)
    }
}

impl Drop for InterruptDisabler {
    fn drop(&mut self) {
        if self.0 {
            crate::csr::sstatus::enable_interrupts();
        }
    }
}

#[track_caller]
pub fn assert_interrupts_disabled() {
    assert_eq!(crate::csr::sstatus::read() & 2, 0, "interrupts not disabled!");
}
