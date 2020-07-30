#![allow(dead_code)]

use crate::locked::Locked;

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

    #[rustfmt::skip]
    unsafe fn init(&mut self) {
        // Disable interrupts
        self.base.add(UART_INT_ENABLE_REG_OFFSET).write_volatile(0x00);
        // Enable DLAB
        self.base.add(UART_LINE_CTRL_REG_OFFSET).write_volatile(0x80);
        // Set baud ratew to 38400
        self.base.add(UART_DATA_REG_OFFSET).write_volatile(0x03);
        self.base.add(UART_INT_ENABLE_REG_OFFSET).write_volatile(0x00);
        // 8 bits, no parity, one stop bit
        self.base.add(UART_LINE_CTRL_REG_OFFSET).write_volatile(0x03);
        // Enable FIFO, clear, with 14-byte threshold
        self.base.add(UART_INT_ID_FIFO_CTRL_REG_OFFSET).write_volatile(0xC7);
        // IRQ enable, RTS/DSR set
        self.base.add(UART_MODEM_CTRL_REG_OFFSET).write_volatile(0x0B);
    }

    pub fn line_status(&self) -> u8 {
        unsafe { self.base.add(UART_LINE_STATUS_REG_OFFSET).read_volatile() }
    }

    pub fn data_waiting(&self) -> bool {
        let value = self.line_status() & 1;

        value == 1
    }

    // TODO: error handling?
    pub fn read(&mut self) -> u8 {
        while !self.data_waiting() {}

        unsafe { self.base.add(UART_DATA_REG_OFFSET).read_volatile() }
    }

    pub fn data_empty(&self) -> bool {
        let value = self.line_status() & (1 << 6);

        value == (1 << 6)
    }

    pub fn write(&mut self, data: u8) {
        while !self.data_empty() {}

        unsafe { self.base.add(UART_DATA_REG_OFFSET).write_volatile(data) }
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

pub struct UartLogger;

impl log::Log for UartLogger {
    #[allow(unused_variables)]
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        #[cfg(debug_assertions)]
        return true;

        #[cfg(not(debug_assertions))]
        return metadata.level() <= log::Level::Info;
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut mod_path = record
                .module_path_static()
                .or_else(|| record.module_path())
                .unwrap_or("<n/a>");

            mod_path = if mod_path == "vanadinite" {
                "vanadinite::main"
            } else {
                mod_path
            };

            #[cfg(debug_assertions)]
            {
                let file = record
                    .file_static()
                    .or_else(|| record.file())
                    .unwrap_or("<n/a>");

                println!(
                    "[ {:>5} ] [{} {}:{}] {}",
                    record.level(),
                    mod_path,
                    file,
                    record.line().unwrap_or(0),
                    record.args()
                );
            }

            #[cfg(not(debug_assertions))]
            println!("[ {:>5} ] [{}] {}", record.level(), mod_path, record.args());
        }
    }

    fn flush(&self) {}
}
