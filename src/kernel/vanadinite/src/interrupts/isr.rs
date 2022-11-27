// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::drivers::generic::plic::{InterruptClaim, Plic};
use crate::sync::SpinRwLock;

const ISR_LIMIT: usize = 128;

static ISR_REGISTRY: [IsrEntry; ISR_LIMIT] = [const { IsrEntry::new() }; ISR_LIMIT];

type DynIsrCallback = dyn Fn(&Plic, InterruptClaim<'_>, usize) -> Result<(), &'static str> + Send + 'static;

#[derive(Debug)]
pub struct IsrEntry {
    f: SpinRwLock<Option<alloc::boxed::Box<DynIsrCallback>>>,
}

impl IsrEntry {
    const fn new() -> Self {
        Self { f: SpinRwLock::new(None) }
    }

    fn set(&self, f: impl Fn(&Plic, InterruptClaim<'_>, usize) -> Result<(), &'static str> + Send + 'static) {
        *self.f.write() = Some(alloc::boxed::Box::new(f));
    }
}

// TODO: move the trait bound to a trait alias when it doesn't cause inference
// issues...
pub fn register_isr<F>(interrupt_id: usize, f: F)
where
    F: Fn(&Plic, InterruptClaim<'_>, usize) -> Result<(), &'static str> + Send + 'static,
{
    log::debug!("Registering ISR for interrupt ID {}", interrupt_id);
    ISR_REGISTRY[interrupt_id].set(f);
}

pub fn invoke_isr(plic: &Plic, claim: InterruptClaim<'_>, interrupt_id: usize) -> Result<(), &'static str> {
    match ISR_REGISTRY[interrupt_id].f.read().as_ref() {
        Some(f) => f(plic, claim, interrupt_id),
        None => Ok(claim.complete()),
    }
}
