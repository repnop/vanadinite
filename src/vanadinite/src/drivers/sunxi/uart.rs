// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{drivers::CompatibleWith, io::ConsoleDevice};

#[repr(C)]
pub struct SunxiUart {
    data: registers::Data,
    interrupt_enable: registers::InterruptEnable,
    line_control: registers::LineControl,
    modem_control: registers::ModemControl,
    line_status: registers::LineStatus,
    modem_status: registers::ModemStatus,
    scratch: registers::Scratch,
    _padding0: [u8; 96],
    status: registers::Status,
    tx_fifo_level: registers::TransmitFifoLevel,
    rx_fifo_level: registers::ReceiveFifoLevel,
    dma_handshake_cfg: registers::DmaHandshakeConfiguration,
    dma_request_enable: registers::DmaRequestEnable,
    _padding1: [u8; 20],
    halt_tx: registers::HaltTransmit,
    _padding2: [u8; 8],
    debug_latch: registers::DebugDivisorLatch,
    _padding3: [u8; 56],
    fifo_clock_control: registers::FifoClockControl,
    _padding4: [u8; 12],
    // TODO: add RXDMA registers
}

impl SunxiUart {
    pub fn init(&self) {
        // As per the user manual, the default clock is 24 MHz, so a divisor of
        // 13 gives us the desired baud rate of 115200
        self.with_alt_registers(|latch, fifo_ctrl| {
            latch.set(13);
            fifo_ctrl.enable_fifo(true);
        });
        self.halt_tx.enable(false);

        self.line_control.parity(None);
        self.line_control.stop_bits(registers::StopBits::One);
        self.line_control.data_length(registers::DataBitLength::Eight);
        self.modem_control.mode(registers::UartMode::Uart);
        self.modem_control.loopback_mode(false);

        self.interrupt_enable.received_data_interrupt(true);
    }

    pub fn with_alt_registers<F>(&self, f: F)
    where
        F: FnOnce(&registers::DivisorLatch, &registers::FifoControl) + 'static,
    {
        self.line_control.divisor_latch_access(true);
        f(unsafe { &*(&self.data as *const _ as *const registers::DivisorLatch) }, unsafe {
            &*(&self.interrupt_enable as *const _ as *const registers::FifoControl)
        });
        self.line_control.divisor_latch_access(false);
    }

    pub fn try_read(&self) -> Option<u8> {
        match self.status.rx_fifo_empty() {
            true => None,
            false => Some(self.data.read()),
        }
    }

    pub fn read(&self) -> u8 {
        while self.status.rx_fifo_empty() {}
        self.data.read()
    }

    pub fn write(&self, b: u8) {
        while self.status.tx_fifo_full() {}
        self.data.write(b);
    }
}

impl ConsoleDevice for SunxiUart {
    fn init(&mut self) {
        (&*self).init();
    }

    fn try_read(&self) -> Option<u8> {
        (&*self).try_read()
    }

    fn read(&self) -> u8 {
        (&*self).read()
    }

    fn write(&mut self, n: u8) {
        (&*self).write(n);
    }
}

impl CompatibleWith for SunxiUart {
    fn compatible_with() -> &'static [&'static str] {
        &["allwinner,sun20i-uart"]
    }
}

mod registers {
    #![allow(dead_code)]

    use crate::utils::volatile::{Volatile, Write};

    #[repr(transparent)]
    pub struct Data(Volatile<u32>);

    impl Data {
        pub fn write(&self, b: u8) {
            self.0.write(b as u32);
        }

        pub fn read(&self) -> u8 {
            self.0.read() as u8
        }
    }

    #[repr(C)]
    pub struct DivisorLatch {
        low: Volatile<u32>,
        high: Volatile<u32>,
    }

    impl DivisorLatch {
        pub fn set(&self, divisor: u16) {
            self.low.write((divisor & 0xFF) as u32);
            self.high.write((divisor >> 8) as u32);
        }
    }

    #[repr(transparent)]
    pub struct InterruptEnable(Volatile<u32>);

    impl InterruptEnable {
        pub fn received_data_interrupt(&self, enable: bool) {
            match enable {
                true => self.0.write(self.0.read() | 1),
                false => self.0.write(self.0.read() & !1),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub enum PendingInterrupt {
        ModemStatus,
        TransmitHoldingEmpty,
        Rs485,
        ReceiveDataAvailable,
        ReceiverLineStatus,
        BusyDetect,
        CharacterTimeout,
    }

    #[repr(transparent)]
    pub struct InterruptIdentity(Volatile<u32>);

    impl InterruptIdentity {
        pub fn fifo_enabled(&self) -> bool {
            self.0.read() & 0b1100_0000 == 0b1100_0000
        }

        pub fn interrupt_id(&self) -> Option<PendingInterrupt> {
            match self.0.read() & 0b1111 {
                0b0000 => Some(PendingInterrupt::ModemStatus),
                0b0001 => None,
                0b0010 => Some(PendingInterrupt::TransmitHoldingEmpty),
                0b0011 => Some(PendingInterrupt::Rs485),
                0b0100 => Some(PendingInterrupt::ReceiveDataAvailable),
                0b0110 => Some(PendingInterrupt::ReceiverLineStatus),
                0b0111 => Some(PendingInterrupt::BusyDetect),
                0b1100 => Some(PendingInterrupt::CharacterTimeout),
                _ => unreachable!(),
            }
        }
    }

    #[repr(transparent)]
    pub struct FifoControl(Volatile<u32, Write>);

    impl FifoControl {
        pub fn enable_fifo(&self, enable: bool) {
            match enable {
                true => self.0.write(1),
                false => self.0.write(0),
            }
        }
    }

    #[repr(u32)]
    pub enum PairtySelect {
        Odd = 0,
        Even = 1,
        ReverseLcr = 2,
    }

    #[repr(u32)]
    pub enum StopBits {
        One = 0,
        Two = 1,
    }

    #[repr(u32)]
    pub enum DataBitLength {
        Five = 0,
        Six = 1,
        Seven = 2,
        Eight = 3,
    }

    #[repr(transparent)]
    pub struct LineControl(Volatile<u32>);

    impl LineControl {
        pub fn divisor_latch_access(&self, enable: bool) {
            match enable {
                true => self.0.write(self.0.read() | (1 << 7)),
                false => self.0.write(self.0.read() & !(1 << 7)),
            }
        }

        /// Enables & sets the parity selection, or disables parity if `None`
        pub fn parity(&self, select: Option<PairtySelect>) {
            match select {
                Some(select) => self.0.write(self.0.read() | ((select as u32) << 4) | (1 << 3)),
                None => self.0.write(self.0.read() & !(1 << 3)),
            }
        }

        pub fn stop_bits(&self, bits: StopBits) {
            self.0.write(self.0.read() | ((bits as u32) << 2));
        }

        pub fn data_length(&self, length: DataBitLength) {
            self.0.write(self.0.read() | (length as u32));
        }
    }

    #[repr(u32)]
    pub enum UartMode {
        Uart = 0,
        IrDaSir = 1,
        Rs485 = 2,
    }

    #[repr(transparent)]
    pub struct ModemControl(Volatile<u32>);

    impl ModemControl {
        pub fn mode(&self, mode: UartMode) {
            self.0.write(self.0.read() | ((mode as u32) << 6));
        }

        pub fn loopback_mode(&self, enable: bool) {
            match enable {
                true => self.0.write(self.0.read() | (1 << 4)),
                false => self.0.write(self.0.read() & !(1 << 4)),
            }
        }
    }

    #[repr(transparent)]
    pub struct LineStatus(Volatile<u32>);

    impl LineStatus {
        pub fn rx_data_fifo_error(&self) -> bool {
            self.0.read() & 0b1000_0000 == 0b1000_0000
        }

        pub fn transmitter_empty(&self) -> bool {
            self.0.read() & 0b0100_0000 == 0b0100_0000
        }

        pub fn tx_holding_empty(&self) -> bool {
            self.0.read() & 0b0010_0000 == 0b0010_0000
        }

        pub fn data_ready(&self) -> bool {
            self.0.read() & 1 == 1
        }
    }

    #[repr(transparent)]
    pub struct ModemStatus(Volatile<u32>);

    #[repr(transparent)]
    pub struct Scratch(Volatile<u32>);

    impl Scratch {
        pub fn write(&self, b: u8) {
            self.0.write(b as u32)
        }

        pub fn read(&self) -> u8 {
            self.0.read() as u8
        }
    }

    #[repr(transparent)]
    pub struct Status(Volatile<u32>);

    impl Status {
        pub fn rx_fifo_full(&self) -> bool {
            self.0.read() & 0b1_0000 == 0b1_0000
        }

        pub fn rx_fifo_empty(&self) -> bool {
            self.0.read() & 0b1000 == 0b0000
        }

        pub fn tx_fifo_full(&self) -> bool {
            self.0.read() & 0b0010 == 0b0000
        }

        pub fn tx_fifo_empty(&self) -> bool {
            self.0.read() & 0b0100 == 0b0100
        }

        pub fn busy(&self) -> bool {
            self.0.read() & 1 == 1
        }
    }

    #[repr(transparent)]
    pub struct TransmitFifoLevel(Volatile<u32>);

    #[repr(transparent)]
    pub struct ReceiveFifoLevel(Volatile<u32>);

    #[repr(transparent)]
    pub struct DmaHandshakeConfiguration(Volatile<u32>);

    #[repr(transparent)]
    pub struct DmaRequestEnable(Volatile<u32>);

    #[repr(transparent)]
    pub struct HaltTransmit(Volatile<u32>);

    impl HaltTransmit {
        pub fn enable(&self, enable: bool) {
            match enable {
                true => self.0.write(self.0.read() | 1),
                false => self.0.write(self.0.read() & !1),
            }
        }
    }

    #[repr(C)]
    pub struct DebugDivisorLatch {
        low: Volatile<u32>,
        high: Volatile<u32>,
    }

    #[repr(transparent)]
    pub struct FifoClockControl(Volatile<u32>);

    #[repr(transparent)]
    pub struct RxdmaControl(Volatile<u32>);

    #[repr(transparent)]
    pub struct RxdmaStart(Volatile<u32>);

    #[repr(transparent)]
    pub struct RxdmaStatus(Volatile<u32>);

    #[repr(transparent)]
    pub struct RxdmaLimit(Volatile<u32>);

    #[repr(C)]
    pub struct RxdmaBufferStartAddress {
        low: Volatile<u32>,
        high: Volatile<u32>,
    }

    #[repr(transparent)]
    pub struct RxdmaBufferLength(Volatile<u32>);

    #[repr(transparent)]
    pub struct RxdmaInterruptEnable(Volatile<u32>);

    #[repr(transparent)]
    pub struct RxdmaInterruptStatus(Volatile<u32>);

    #[repr(C)]
    pub struct RxdmaBufferWriteAddress {
        low: Volatile<u32>,
        high: Volatile<u32>,
    }

    #[repr(C)]
    pub struct RxdmaBufferReadAddress {
        low: Volatile<u32>,
        high: Volatile<u32>,
    }

    #[repr(transparent)]
    pub struct RxdmaDataCount(Volatile<u32>);
}
