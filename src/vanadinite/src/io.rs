// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    drivers::{generic::uart16550::Uart16550, sifive::fu540_c000::uart::SiFiveUart, CompatibleWith},
    sync::Mutex,
};
use core::cell::UnsafeCell;

pub trait ConsoleDevice: 'static {
    fn init(&mut self);
    fn read(&self) -> u8;
    fn write(&mut self, n: u8);
}

impl core::fmt::Write for dyn ConsoleDevice {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.as_bytes() {
            self.write(*byte);
        }

        Ok(())
    }
}

pub struct StaticConsoleDevice(Option<&'static UnsafeCell<dyn ConsoleDevice>>);

impl StaticConsoleDevice {
    unsafe fn new<T: ConsoleDevice>(inner: *mut T) -> Self {
        let inner = {
            let mut_ref = (&mut *inner) as &'static mut dyn ConsoleDevice;
            mut_ref.init();

            &*(mut_ref as *mut _ as *mut _)
        };
        Self(Some(inner))
    }
}

impl core::fmt::Write for StaticConsoleDevice {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if let Some(console) = self.0 {
            let console = unsafe { &mut *console.get() };
            for byte in s.as_bytes() {
                console.write(*byte);
            }
        }

        Ok(())
    }
}

impl ConsoleDevice for StaticConsoleDevice {
    fn init(&mut self) {
        if let Some(inner) = &mut self.0 {
            unsafe { &mut *inner.get() }.init();
        }
    }

    fn read(&self) -> u8 {
        if let Some(inner) = &self.0 {
            return unsafe { &*inner.get() }.read();
        }

        0
    }

    fn write(&mut self, n: u8) {
        if let Some(inner) = &mut self.0 {
            unsafe { &mut *inner.get() }.write(n);
        }
    }
}

unsafe impl Send for StaticConsoleDevice {}
unsafe impl Sync for StaticConsoleDevice {}

pub static CONSOLE: Mutex<StaticConsoleDevice> = Mutex::new(StaticConsoleDevice(None));

/// # Safety
/// The given pointer must be a valid object in memory
///
/// The device will also have `.init()` called on it.
pub unsafe fn set_console<T: ConsoleDevice>(device: *mut T) {
    *CONSOLE.lock() = StaticConsoleDevice::new(device);
}

pub enum ConsoleDevices {
    Uart16550,
    SiFiveUart,
}

impl ConsoleDevices {
    pub fn from_compatible(ptr: *mut u8, compatible: fdt::Compatible<'_>) -> Option<Self> {
        if compatible.all().any(|s| Uart16550::list().contains(&s)) {
            Some(ConsoleDevices::Uart16550)
        } else if compatible.all().any(|s| SiFiveUart::list().contains(&s)) {
            Some(ConsoleDevices::SiFiveUart)
        } else {
            None
        }
    }

    pub fn set_console(self, ptr: *mut u8) {
        match self {
            ConsoleDevices::Uart16550 => unsafe { set_console(ptr as *mut Uart16550) },
            ConsoleDevices::SiFiveUart => unsafe { set_console(ptr as *mut SiFiveUart) },
        }
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    CONSOLE.lock().write_fmt(args).unwrap();
}

struct Logger;

impl log::Log for Logger {
    #[allow(unused_variables)]
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        #[cfg(any(debug_assertions, feature = "debug_log"))]
        return true;

        #[cfg(all(not(debug_assertions), not(feature = "debug_log")))]
        return metadata.level() <= log::Level::Info;
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut mod_path = record.module_path_static().or_else(|| record.module_path()).unwrap_or("<n/a>");

            mod_path = if mod_path == "vanadinite" { "vanadinite::main" } else { mod_path };

            #[cfg(debug_assertions)]
            {
                let file = record.file_static().or_else(|| record.file()).unwrap_or("<n/a>");

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

pub fn init_logging() {
    log::set_logger(&Logger).expect("failed to init logging");
    //#[cfg(debug_assertions)]
    log::set_max_level(log::LevelFilter::Trace);
    //#[cfg(not(debug_assertions))]
    //log::set_max_level(log::LevelFilter::Info);
}
