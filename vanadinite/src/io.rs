use crate::sync::Mutex;
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
        #[cfg(debug_assertions)]
        return true;

        #[cfg(not(debug_assertions))]
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
    #[cfg(debug_assertions)]
    log::set_max_level(log::LevelFilter::Trace);
    #[cfg(not(debug_assertions))]
    log::set_max_level(log::LevelFilter::Info);
}
