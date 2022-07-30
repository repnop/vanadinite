// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::drivers::CompatibleWith;
pub use registers::InterruptClaim;
use volatile::{Read, ReadWrite, Volatile};

#[repr(C)]
pub struct Plic {
    source_priorities: [registers::Priority; 1024],
    interrupt_pending: registers::InterruptPending,
    _padding1: [u8; 3968],
    interrupt_enable: [registers::Context<registers::InterruptEnable>; 15872],
    _padding2: [u8; 57344],
    threshold_and_claim: [registers::Context<registers::ThresholdAndClaim>; 15872],
    _padding3: [u8; 8184],
}

impl Plic {
    pub fn init(&self, max_interrupts: usize, contexts: impl Iterator<Item = usize>) {
        for i in 1..max_interrupts {
            self.source_priorities[i].set(0);
        }

        for context in contexts {
            for i in 0..max_interrupts {
                self.interrupt_enable[context].disable(i);
            }

            self.threshold_and_claim[context].priority_threshold.set(0);
        }
    }

    pub fn enable_interrupt(&self, context: usize, source: usize) {
        log::debug!("Enabling interrupt {}", source);
        self.interrupt_enable[context].enable(source);
    }

    pub fn disable_interrupt(&self, context: usize, source: usize) {
        log::debug!("Disabling interrupt {}", source);
        self.interrupt_enable[context].disable(source);
    }

    pub fn set_interrupt_priority(&self, source: usize, mut priority: usize) {
        if priority > Self::max_priority() {
            log::warn!("Priority provided for source {} exceeds max priority value, setting to max", source);
            priority = Self::max_priority();
        }

        log::debug!("Setting priority {} for source {}", priority, source);
        self.source_priorities[source].set(priority as u32)
    }

    pub fn set_context_threshold(&self, context: usize, mut threshold: usize) {
        if threshold > Self::max_priority() {
            log::warn!("Threshold provided for context {} exceeds max priority value, setting to max", context);
            threshold = Self::max_priority();
        }

        log::debug!("Setting threshold {} for context {}", threshold, context);
        self.threshold_and_claim[context].priority_threshold.set(threshold as u32)
    }

    pub fn is_pending(&self, source: usize) -> bool {
        self.interrupt_pending.is_pending(source)
    }

    pub fn claim(&self, context: usize) -> Option<registers::InterruptClaim<'_>> {
        self.threshold_and_claim[context].claim_complete.claim()
    }

    pub fn complete(&self, context: usize, interrupt_id: usize) {
        self.threshold_and_claim[context].claim_complete.complete(interrupt_id);
    }

    pub const fn max_priority() -> usize {
        #[cfg(all(not(feature = "platform.virt"), not(feature = "platform.sifive_u")))]
        compile_error!("Update PLIC max priority for new platform");

        // This value is fixed for the platforms we currently support, but may
        // need `#[cfg]`'d in the future
        7
    }
}

mod registers {
    use super::*;

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct Context<T>(T);

    impl<T> core::ops::Deref for Context<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct Priority(Volatile<u32, ReadWrite>);

    impl Priority {
        pub fn set(&self, priority: u32) {
            self.0.write(priority);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptPending(Volatile<[u32; 32], Read>);

    impl InterruptPending {
        pub fn is_pending(&self, interrupt_id: usize) -> bool {
            let (u32_index, bit_index) = (interrupt_id / 32, interrupt_id % 32);
            (self.0[u32_index].read() >> bit_index) & 1 == 1
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptEnable(Volatile<[u32; 32], ReadWrite>);

    impl InterruptEnable {
        pub fn enable(&self, interrupt_id: usize) {
            let (u32_index, bit_index) = (interrupt_id / 32, interrupt_id % 32);

            let val = self.0[u32_index].read() | (1 << bit_index);
            self.0[u32_index].write(val);
        }

        pub fn disable(&self, interrupt_id: usize) {
            let (u32_index, bit_index) = (interrupt_id / 32, interrupt_id % 32);

            let val = self.0[u32_index].read() & !(1 << bit_index);
            self.0[u32_index].write(val);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct PriorityThreshold(Volatile<u32, ReadWrite>);

    impl PriorityThreshold {
        pub fn set(&self, priority: u32) {
            self.0.write(priority);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct ClaimComplete(Volatile<u32, ReadWrite>);

    impl ClaimComplete {
        pub fn claim(&self) -> Option<InterruptClaim<'_>> {
            match self.0.read() as usize {
                0 => None,
                interrupt_id => Some(InterruptClaim { interrupt_id, register: self }),
            }
        }

        // Don't make this public to other consumers, they either need to
        // complete the claim as normal or go through the PLIC method explicitly
        pub(super) fn complete(&self, interrupt_id: usize) {
            self.0.write(interrupt_id as u32);
        }
    }

    #[derive(Debug)]
    #[must_use]
    pub struct InterruptClaim<'a> {
        interrupt_id: usize,
        register: &'a ClaimComplete,
    }

    impl InterruptClaim<'_> {
        pub fn interrupt_id(&self) -> usize {
            self.interrupt_id
        }

        pub fn complete(self) {
            // Casting back here is fine because we don't let the user change
            // the interrupt id
            self.register.0.write(self.interrupt_id as u32);
        }
    }

    #[repr(C)]
    pub struct ThresholdAndClaim {
        pub priority_threshold: PriorityThreshold,
        pub claim_complete: ClaimComplete,
        _reserved: [u8; 4088],
    }
}

impl CompatibleWith for Plic {
    fn compatible_with() -> &'static [&'static str] {
        &["riscv,plic0"]
    }
}
