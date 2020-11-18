// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    drivers::{self, CompatibleWith, EnableMode},
    utils::volatile::{Read, ReadWrite, Volatile},
};

#[repr(C)]
pub struct Plic {
    pub source_priorities: [registers::Priority; 1024],
    pub interrupt_pending: registers::InterruptPending,
    _padding1: [u8; 3968],
    pub interrupt_enable: [registers::Context<registers::InterruptEnable>; 15872],
    _padding2: [u8; 57344],
    pub threshold_and_claim: [registers::Context<registers::ThresholdAndClaim>; 15872],
    _padding3: [u8; 8184],
}

impl Plic {
    // FIXME: actually do initialization
    pub fn init(&self, num_interrupts: usize) {
        // for i in 0..1023 {
        //     self.source_priorities[i].set(1);
        // }
        // //
        // for context in 1..=1 {
        //     self.interrupt_enable[context].init();
        //     self.threshold_and_claim[context].priority_threshold.init();
        // }
        // self.threshold_and_claim[0].priority_threshold.init();
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
        pub fn get(&self) -> u32 {
            self.0.read()
        }

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
        pub fn get(&self) -> u32 {
            self.0.read()
        }

        pub fn set(&self, priority: u32) {
            log::info!("{:#p}, {:#x}", &self.0, priority);
            self.0.write(priority);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct ClaimComplete(Volatile<u32, ReadWrite>);

    impl ClaimComplete {
        pub fn claim(&self) -> Option<usize> {
            match self.0.read() {
                0 => None,
                n => Some(n as usize),
            }
        }

        pub fn complete(&self, interrupt_id: usize) {
            self.0.write(interrupt_id as u32);
        }
    }

    // Neat idea, but currently can't fit it into the design, oh well.
    // #[derive(Debug)]
    // pub struct InterruptClaim<'a> {
    //     interrupt_id: u32,
    //     register: &'a ClaimComplete,
    // }
    //
    // impl InterruptClaim<'_> {
    //     pub fn source(&self) -> u32 {
    //         self.interrupt_id
    //     }
    //
    //     pub fn complete(self) {
    //         self.register.0.write(self.interrupt_id);
    //     }
    // }

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

impl drivers::Plic for Plic {
    fn enable_interrupt(&self, mode: EnableMode, source: usize) {
        log::info!("Enabling interrupt {}", source);
        match mode {
            EnableMode::Local => self.interrupt_enable[current_context()].enable(source),
            EnableMode::Global => todo!("plic global enable_interrupt"),
        }
    }

    fn disable_interrupt(&self, mode: EnableMode, source: usize) {
        match mode {
            EnableMode::Local => self.interrupt_enable[current_context()].disable(source),
            EnableMode::Global => todo!("plic global enable_interrupt"),
        }
    }

    fn interrupt_priority(&self, source: usize, priority: usize) {
        log::info!("Setting priority {} for source {}", priority, source);
        self.source_priorities[source].set(priority as u32)
    }

    fn context_threshold(&self, mode: EnableMode, threshold: usize) {
        log::info!("Setting threshold {} for context {}", threshold, current_context());
        match mode {
            EnableMode::Local => self.threshold_and_claim[current_context()].priority_threshold.set(threshold as u32),
            EnableMode::Global => todo!("plic global enable_interrupt"),
        }
    }

    fn is_pending(&self, source: usize) -> bool {
        self.interrupt_pending.is_pending(source)
    }

    fn claim(&self) -> Option<usize> {
        self.threshold_and_claim[current_context()].claim_complete.claim()
    }

    fn complete(&self, source: usize) {
        self.threshold_and_claim[current_context()].claim_complete.complete(source)
    }
}

// FIXME: this is kind of hacky because contexts aren't currently standardized,
// should look for a better way to do it in the future
pub fn current_context() -> usize {
    2 * crate::hart_local::hart_local_info().hart_id() + 1
}
