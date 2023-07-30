// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod isr;

use crate::drivers::generic::plic;
use crate::sync::SpinMutex;
use crate::utils::SameHartDeadlockDetection;

pub static PLIC: SpinMutex<Option<&'static plic::Plic>, SameHartDeadlockDetection> =
    SpinMutex::new(None, SameHartDeadlockDetection::new());

pub fn register_plic(plic: &'static plic::Plic) {
    *PLIC.lock() = Some(plic);
}

pub struct InterruptDisabler(bool);

impl InterruptDisabler {
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
