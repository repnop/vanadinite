// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{drivers::CompatibleWith, utils::Volatile};

#[repr(C)]
pub struct Uart16550 {
    data_register: Volatile<u8>,
    interrupt_enable: Volatile<u8>,
    int_id_fifo_control: Volatile<u8>,
    line_control: Volatile<u8>,
    modem_control: Volatile<u8>,
    line_status: Volatile<u8>,
    modem_status: Volatile<u8>,
    scratch: Volatile<u8>,
}

impl Uart16550 {
    pub fn init(&mut self) {
        self.interrupt_enable.write(0x00);
        self.line_control.write(0x80);
        self.data_register.write(0x03);
        self.line_control.write(0x03);
        self.int_id_fifo_control.write(0xC7);
        self.modem_control.write(0x0B);
    }

    pub fn line_status(&self) -> u8 {
        self.line_status.read()
    }

    pub fn data_waiting(&self) -> bool {
        let value = self.line_status() & 1;

        value == 1
    }

    // TODO: error handling?
    pub fn read(&self) -> u8 {
        while !self.data_waiting() {}

        self.data_register.read()
    }

    pub fn try_read(&self) -> Option<u8> {
        if !self.data_waiting() {
            return None;
        }

        Some(self.data_register.read())
    }

    pub fn data_empty(&self) -> bool {
        let value = self.line_status() & (1 << 6);

        value == (1 << 6)
    }

    pub fn write(&mut self, data: u8) {
        while !self.data_empty() {}

        self.data_register.write(data)
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write(byte);
        }
    }
}

impl core::fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

impl crate::io::ConsoleDevice for Uart16550 {
    fn init(&mut self) {
        self.init();
    }

    fn read(&self) -> u8 {
        self.read()
    }

    fn write(&mut self, n: u8) {
        self.write(n)
    }
}

impl CompatibleWith for Uart16550 {
    fn list() -> &'static [&'static str] {
        &["ns16550", "ns16550a"]
    }
}
