// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Debug)]
#[repr(C)]
pub struct SiFiveUart {
    tx_data: registers::TxData,
    rx_data: registers::RxData,
    tx_control: registers::TxCtrl,
    rx_control: registers::RxCtrl,
    interrupt_enable: registers::InterruptEnable,
    interrupt_pending: registers::InterruptPending,
    baud_rate_divisor: registers::BaudDivisor,
}

impl SiFiveUart {
    pub fn init(&mut self) {
        // Enable receive
        self.rx_control.rx_enable(true);
        // Enable transmit
        self.tx_control.tx_enable(true);
        self.tx_control.extra_stop_bit(false);
        // Disable interrupts
        self.interrupt_enable.rx_watermark_enable(false);
        self.interrupt_enable.tx_watermark_enable(false);
        // Set baud rate to 31250 Hz
        self.baud_rate_divisor.divisor(16000);
    }

    pub fn read(&self) -> u8 {
        while self.rx_data.is_empty() {}

        self.rx_data.read()
    }

    pub fn write(&mut self, n: u8) {
        while self.tx_data.is_full() {}

        self.tx_data.write(n);
    }
}

impl crate::io::ConsoleDevice for SiFiveUart {
    fn init(&mut self) {
        self.init();
    }

    fn read(&self) -> u8 {
        self.read()
    }

    fn write(&mut self, n: u8) {
        self.write(n);
    }
}

impl crate::drivers::CompatibleWith for SiFiveUart {
    fn list() -> &'static [&'static str] {
        &["sifive,uart0"]
    }
}

mod registers {
    use crate::utils::volatile::Volatile;
    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TxData(Volatile<u32>);

    impl TxData {
        pub fn write(&mut self, val: u8) {
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
        pub fn read(&self) -> u8 {
            self.read() as u8
        }

        pub fn is_empty(&self) -> bool {
            self.0.read() >> 31 == 1
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TxCtrl(Volatile<u32>);

    impl TxCtrl {
        pub fn tx_enable(&mut self, enable: bool) {
            let val = self.0.read() | (enable as u32);
            self.0.write(val);
        }

        pub fn extra_stop_bit(&mut self, enable: bool) {
            let val = self.0.read() | ((enable as u32) << 1);
            self.0.write(val);
        }

        pub fn watermark_level(&mut self, watermark: u8) {
            let val = self.0.read() | ((watermark as u32 & 0b111) << 16);
            self.0.write(val);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct RxCtrl(Volatile<u32>);

    impl RxCtrl {
        pub fn rx_enable(&mut self, enable: bool) {
            let val = self.0.read() | (enable as u32);
            self.0.write(val);
        }

        pub fn watermark_level(&mut self, watermark: u8) {
            let val = self.0.read() | ((watermark as u32 & 0b111) << 16);
            self.0.write(val);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptEnable(Volatile<u32>);

    impl InterruptEnable {
        pub fn tx_watermark_enable(&mut self, enable: bool) {
            let val = self.0.read() | (enable as u32);
            self.0.write(val);
        }

        pub fn rx_watermark_enable(&mut self, enable: bool) {
            let val = self.0.read() | ((enable as u32) << 1);
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
        pub fn divisor(&mut self, divisor: u16) {
            self.0.write(divisor as u32);
        }
    }
}
