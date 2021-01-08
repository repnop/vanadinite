// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{drivers::CompatibleWith, drivers::InterruptServicable, io::ConsoleDevice};
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
#[repr(C)]
pub struct SifiveUart {
    tx_data: registers::TxData,
    rx_data: registers::RxData,
    tx_control: registers::TxCtrl,
    rx_control: registers::RxCtrl,
    interrupt_enable: registers::InterruptEnable,
    interrupt_pending: registers::InterruptPending,
    baud_rate_divisor: registers::BaudDivisor,
}

impl SifiveUart {
    pub fn init(&self) {
        // Enable receive
        self.rx_control.rx_enable(true);
        // Enable transmit
        self.tx_control.tx_enable(true);

        self.tx_control.extra_stop_bit(false);
        self.rx_control.watermark_level(1);

        // Set interrupt enables
        self.interrupt_enable.rx_watermark_enable(true);
        self.interrupt_enable.tx_watermark_enable(false);

        // Set baud rate to 31250 Hz
        self.baud_rate_divisor.divisor(16000);
    }

    pub fn read(&self) -> u8 {
        loop {
            match self.rx_data.try_read() {
                Some(c) => break c,
                None => continue,
            }
        }
    }

    pub fn write(&self, n: u8) {
        static LAST_WROTE_NEWLINE: AtomicBool = AtomicBool::new(false);

        while self.tx_data.is_full() {}

        self.tx_data.write(n);

        if n == b'\n' {
            LAST_WROTE_NEWLINE.store(true, Ordering::SeqCst);
            self.write(b'\r');
        } else if n == b'\r' {
            if !LAST_WROTE_NEWLINE.load(Ordering::SeqCst) {
                self.write(b'\n');
            }

            LAST_WROTE_NEWLINE.store(false, Ordering::SeqCst);
        }
    }
}

impl ConsoleDevice for SifiveUart {
    fn init(&mut self) {
        (&*self).init();
    }

    fn read(&self) -> u8 {
        self.read()
    }

    fn write(&mut self, n: u8) {
        (&*self).write(n);
    }
}

impl CompatibleWith for SifiveUart {
    fn compatible_with() -> &'static [&'static str] {
        &["sifive,uart0"]
    }
}

impl InterruptServicable for SifiveUart {
    fn isr(_: usize, private: usize) -> Result<(), &'static str> {
        let this: &'static Self = unsafe { &*(private as *const _) };
        let _ = crate::io::INPUT_QUEUE.push(this.read());

        Ok(())
    }
}

mod registers {
    use crate::utils::volatile::Volatile;
    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TxData(Volatile<u32>);

    impl TxData {
        pub fn write(&self, val: u8) {
            self.0.write(val as u32);
        }

        pub fn is_full(&self) -> bool {
            self.0.read() >> 31 == 1
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct RxData(Volatile<u32>);

    impl RxData {
        pub fn try_read(&self) -> Option<u8> {
            let read = self.0.read();
            if read >> 31 == 0 {
                Some(read as u8)
            } else {
                None
            }
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TxCtrl(Volatile<u32>);

    impl TxCtrl {
        pub fn tx_enable(&self, enable: bool) {
            let val = (self.0.read() & !1) | (enable as u32);
            self.0.write(val);
        }

        pub fn extra_stop_bit(&self, enable: bool) {
            let val = (self.0.read() & !2) | ((enable as u32) << 1);
            self.0.write(val);
        }

        pub fn watermark_level(&self, watermark: u8) {
            let val = (self.0.read() & !(0b111 << 16)) | ((watermark as u32 & 0b111) << 16);
            self.0.write(val);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct RxCtrl(pub Volatile<u32>);

    impl RxCtrl {
        pub fn rx_enable(&self, enable: bool) {
            let val = (self.0.read() & !1) | (enable as u32);
            self.0.write(val);
        }

        pub fn watermark_level(&self, watermark: u8) {
            let val = (self.0.read() & !(0b111 << 16)) | ((watermark as u32 & 0b111) << 16);
            self.0.write(val);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptEnable(Volatile<u32>);

    impl InterruptEnable {
        pub fn tx_watermark_enable(&self, enable: bool) {
            let val = (self.0.read() & !1) | (enable as u32);
            self.0.write(val);
        }

        pub fn rx_watermark_enable(&self, enable: bool) {
            let val = (self.0.read() & !2) | ((enable as u32) << 1);
            self.0.write(val);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptPending(Volatile<u32>);

    impl InterruptPending {
        pub fn tx_watermark_pending(&self) -> bool {
            self.0.read() & 1 == 1
        }

        pub fn rx_watermark_pending(&self) -> bool {
            (self.0.read() >> 1) & 1 == 1
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct BaudDivisor(Volatile<u32>);

    impl BaudDivisor {
        pub fn divisor(&self, divisor: u16) {
            self.0.write(divisor as u32);
        }
    }
}
