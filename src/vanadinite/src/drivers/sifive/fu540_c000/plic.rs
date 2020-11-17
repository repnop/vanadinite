// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::utils::volatile::{Read, ReadWrite, Volatile};
pub use registers::{InterruptSource, PriorityLevel};

#[repr(C)]
pub struct Plic {
    _reserved1: u32,
    pub source_priorities: [registers::Priority; 53],
    _reserved2: [u8; 3878],
    pub interrupt_pending: registers::InterruptPending,
    _unused1: [u8; 4344],
    pub hart_1_interrupt_enable: registers::InterruptEnable,
    _unused2: [u8; 248],
    pub hart_2_interrupt_enable: registers::InterruptEnable,
    _unused3: [u8; 248],
    pub hart_3_interrupt_enable: registers::InterruptEnable,
    _unused4: [u8; 248],
    pub hart_4_interrupt_enable: registers::InterruptEnable,
    _unused5: [u8; 2096120],
    pub hart_1_priority_threshold: registers::PriorityThreshold,
    pub hart_1_claim_complete: registers::ClaimComplete,
    _unused6: [u8; 8184],
    pub hart_2_priority_threshold: registers::PriorityThreshold,
    pub hart_2_claim_complete: registers::ClaimComplete,
    _unused7: [u8; 8184],
    pub hart_3_priority_threshold: registers::PriorityThreshold,
    pub hart_3_claim_complete: registers::ClaimComplete,
    _unused8: [u8; 8184],
    pub hart_4_priority_threshold: registers::PriorityThreshold,
    pub hart_4_claim_complete: registers::ClaimComplete,
}

mod registers {
    use super::*;

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct Priority(Volatile<u32, ReadWrite>);

    impl Priority {
        pub fn get(&self) -> PriorityLevel {
            unsafe { core::mem::transmute(self.0.read() & 0b111) }
        }

        pub fn set(&self, priority: PriorityLevel) {
            self.0.write(priority as u32);
        }
    }

    #[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
    #[repr(u32)]
    pub enum PriorityLevel {
        Never = 0,
        One = 1,
        Two = 2,
        Three = 3,
        Four = 4,
        Five = 5,
        Six = 6,
        Seven = 7,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum InterruptSource {
        L2Cache(usize),
        Uart0,
        Uart1,
        Qspi2,
        Gpio(usize),
        Dma(usize),
        DdrSubsystem,
        ChiplinkMsi(usize),
        Pwm0(usize),
        Pwm1(usize),
        I2C,
        Qspi0,
        Qspi1,
        GigabitEthernet,
    }

    impl InterruptSource {
        pub fn to_u32(self) -> u32 {
            match self {
                InterruptSource::L2Cache(n) => n as u32 + 1,
                InterruptSource::Uart0 => 4,
                InterruptSource::Uart1 => 5,
                InterruptSource::Qspi2 => 6,
                InterruptSource::Gpio(n) => n as u32 + 7,
                InterruptSource::Dma(n) => n as u32 + 23,
                InterruptSource::DdrSubsystem => 31,
                InterruptSource::ChiplinkMsi(n) => n as u32 + 32,
                InterruptSource::Pwm0(n) => n as u32 + 42,
                InterruptSource::Pwm1(n) => n as u32 + 46,
                InterruptSource::I2C => 50,
                InterruptSource::Qspi0 => 51,
                InterruptSource::Qspi1 => 52,
                InterruptSource::GigabitEthernet => 53,
            }
        }

        fn from_u32(n: u32) -> Self {
            assert!((1..=53).contains(&n), "bad interrupt source");

            let n = n as usize;
            match n {
                1..=3 => InterruptSource::L2Cache(n - 1),
                4 => InterruptSource::Uart0,
                5 => InterruptSource::Uart1,
                6 => InterruptSource::Qspi2,
                7..=22 => InterruptSource::Gpio(n - 7),
                23..=30 => InterruptSource::Dma(n - 23),
                31 => InterruptSource::DdrSubsystem,
                32..=41 => InterruptSource::ChiplinkMsi(n - 32),
                42..=45 => InterruptSource::Pwm0(n - 42),
                46..=49 => InterruptSource::Pwm1(n - 46),
                50 => InterruptSource::I2C,
                51 => InterruptSource::Qspi0,
                52 => InterruptSource::Qspi1,
                53 => InterruptSource::GigabitEthernet,
                _ => unreachable!(),
            }
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptPending(Volatile<[u32; 2], Read>);

    impl InterruptPending {
        pub fn is_pending(&self, interrupt_id: usize) -> bool {
            assert!(interrupt_id < 54, "bad interrupt ID");

            match interrupt_id {
                0..=31 => (self.0[0].read() >> interrupt_id) & 1 == 1,
                _ => (self.0[1].read() >> (interrupt_id - 32)) & 1 == 1,
            }
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptEnable(Volatile<[u32; 2], ReadWrite>);

    impl InterruptEnable {
        pub fn enable(&self, interrupt_source: InterruptSource) {
            match interrupt_source.to_u32() {
                n @ 0..=31 => {
                    let val = self.0[0].read() | (1 << n);
                    self.0[0].write(val);
                }
                n => {
                    let val = self.0[1].read() | (1 << (n - 32));
                    self.0[1].write(val);
                }
            }
        }

        pub fn disable(&self, interrupt_source: InterruptSource) {
            match interrupt_source.to_u32() {
                n @ 0..=31 => {
                    let val = self.0[0].read() & !(1 << n);
                    self.0[0].write(val);
                }
                n => {
                    let val = self.0[1].read() & !(1 << (n - 32));
                    self.0[1].write(val);
                }
            }
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct PriorityThreshold(Volatile<u32, ReadWrite>);

    impl PriorityThreshold {
        pub fn get(&self) -> PriorityLevel {
            unsafe { core::mem::transmute(self.0.read() & 0b111) }
        }

        pub fn set(&self, priority: PriorityLevel) {
            self.0.write(priority as u32);
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
        pub fn source(&self) -> InterruptSource {
            InterruptSource::from_u32(self.interrupt_id)
        }

        pub fn complete(self) {
            self.register.0.write(self.interrupt_id);
        }
    }
}
