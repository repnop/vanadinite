#![allow(dead_code)]

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::virt::uart::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    UART0.lock().write_fmt(args).unwrap();
}

use crate::locked::Locked;
use core::ptr::{read_volatile, write_volatile};

const UART_DATA_REG_OFFSET: usize = 0;
const UART_INT_ENABLE_REG_OFFSET: usize = 1;
const UART_INT_ID_FIFO_CTRL_REG_OFFSET: usize = 2;
const UART_LINE_CTRL_REG_OFFSET: usize = 3;
const UART_MODEM_CTRL_REG_OFFSET: usize = 4;
const UART_LINE_STATUS_REG_OFFSET: usize = 5;
const UART_MODEM_STATUS_REG_OFFSET: usize = 6;
const UART_SCRATCH_REG_OFFSET: usize = 7;

lazy_static::lazy_static! {
    pub static ref UART0: Locked<Uart16550> = Locked::new(unsafe { let mut uart = Uart16550::new(); uart.init(); uart });
}

pub struct Uart16550 {
    base: *mut u8,
}

unsafe impl Send for Uart16550 {}

impl Uart16550 {
    const unsafe fn new() -> Uart16550 {
        let base = 0x1000_0000 as *mut u8;

        Self { base }
    }

    unsafe fn init(&mut self) {
        // Disable interrupts
        write_volatile(self.base.add(UART_INT_ENABLE_REG_OFFSET), 0x00);
        // Enable DLAB
        write_volatile(self.base.add(UART_LINE_CTRL_REG_OFFSET), 0x80);
        // Set baud ratew to 38400
        write_volatile(self.base.add(UART_DATA_REG_OFFSET), 0x03);
        write_volatile(self.base.add(UART_INT_ENABLE_REG_OFFSET), 0x00);
        // 8 bits, no parity, one stop bit
        write_volatile(self.base.add(UART_LINE_CTRL_REG_OFFSET), 0x03);
        // Enable FIFO, clear, with 14-byte threshold
        write_volatile(self.base.add(UART_INT_ID_FIFO_CTRL_REG_OFFSET), 0xC7);
        // IRQ enable, RTS/DSR set
        write_volatile(self.base.add(UART_MODEM_CTRL_REG_OFFSET), 0x0B);
    }

    pub fn line_status(&self) -> u8 {
        unsafe { read_volatile(self.base.add(UART_LINE_STATUS_REG_OFFSET)) }
    }

    pub fn data_waiting(&self) -> bool {
        let value = self.line_status() & 1;

        value == 1
    }

    // TODO: error handling?
    pub fn read(&mut self) -> u8 {
        while !self.data_waiting() {}

        unsafe { read_volatile(self.base.add(UART_DATA_REG_OFFSET)) }
    }

    pub fn data_empty(&self) -> bool {
        let value = self.line_status() & (1 << 6);

        value == (1 << 6)
    }

    pub fn write(&mut self, data: u8) {
        while !self.data_empty() {}

        unsafe { write_volatile(self.base.add(UART_DATA_REG_OFFSET), data) }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write(byte);
        }
    }
}

impl core::fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
