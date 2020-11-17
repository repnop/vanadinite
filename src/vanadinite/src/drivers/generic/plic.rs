// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    drivers::CompatibleWith,
    utils::volatile::{Read, ReadWrite, Volatile},
};

#[repr(C)]
pub struct Plic {
    _reserved1: u32,
    pub source_priorities: [registers::Priority; 1023],
    pub interrupt_pending: registers::InterruptPending,
    _padding1: [u8; 3968],
    pub interrupt_enable: [registers::Context<registers::InterruptEnable>; 15872],
    _padding2: [u8; 57468],
    pub threshold_and_claim: [registers::Context<registers::ThresholdAndClaim>; 15872],
    _padding3: [u8; 8184],
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
            self.0.write(priority);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct ClaimComplete(Volatile<u32, ReadWrite>);

    impl ClaimComplete {
        pub fn claim(&self) -> Option<InterruptClaim<'_>> {
            match core::num::NonZeroU32::new(self.0.read()) {
                None => None,
                Some(n) => Some(InterruptClaim { interrupt_id: n.get(), register: self }),
            }
        }
    }

    #[derive(Debug)]
    pub struct InterruptClaim<'a> {
        interrupt_id: u32,
        register: &'a ClaimComplete,
    }

    impl InterruptClaim<'_> {
        pub fn source(&self) -> u32 {
            self.interrupt_id
        }

        pub fn complete(self) {
            self.register.0.write(self.interrupt_id);
        }
    }

    #[repr(C)]
    pub struct ThresholdAndClaim {
        pub priority_threshold: PriorityThreshold,
        pub claim_complete: ClaimComplete,
    }
}

impl CompatibleWith for Plic {
    fn list() -> &'static [&'static str] {
        &["riscv,plic0"]
    }
}

// FIXME: this is kind of hacky because contexts aren't currently standardized,
// should look for a better way to do it in the future
pub fn current_context() -> usize {
    2 * crate::hart_local::hart_local_info().hart_id + 1
}
