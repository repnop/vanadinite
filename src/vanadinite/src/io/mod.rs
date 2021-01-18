// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod block_device;
pub mod console;

use crate::sync::RwLock;
use alloc::{collections::BTreeMap, string::String};
pub use console::*;
use core::fmt::Write;
use crossbeam_queue::ArrayQueue;

pub static INPUT_QUEUE: crate::sync::Lazy<ArrayQueue<u8>> = crate::sync::Lazy::new(|| ArrayQueue::new(4096));

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
    CONSOLE.lock().write_fmt(args).unwrap();
}

pub static LOG_FILTER: RwLock<Option<BTreeMap<String, Option<log::Level>>>> = RwLock::new(None);

pub fn parse_log_filter(filter: Option<&str>) {
    if let Some(filter) = filter {
        let mut map = BTreeMap::new();
        for part in filter.split(',') {
            let mut parts = part.split('=');
            let name = parts.next().unwrap();
            let level = match parts.next() {
                Some("debug") => Some(log::Level::Debug),
                Some("trace") => Some(log::Level::Trace),
                Some("info") => Some(log::Level::Info),
                Some("warn") => Some(log::Level::Warn),
                Some("error") => Some(log::Level::Error),
                None => None,
                Some(s) => {
                    log::warn!("Bad level filter: '{}', skipping", s);
                    continue;
                }
            };

            map.insert(String::from(name), level);
        }

        *LOG_FILTER.write() = Some(map);
    }
}

struct Logger;

impl log::Log for Logger {
    #[allow(unused_variables)]
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        #[cfg(any(debug_assertions, feature = "debug_log"))]
        let min_level = log::Level::Debug;

        #[cfg(all(not(debug_assertions), not(feature = "debug_log")))]
        let min_level = log::Level::Info;

        let mut mod_path = metadata.target();
        mod_path = if mod_path == "vanadinite" { "kmain" } else { mod_path.trim_start_matches("vanadinite::") };

        let filter = LOG_FILTER.read();
        match &*filter {
            Some(filters) => {
                let mod_filter = filters.iter().find(|(k, _)| mod_path.starts_with(*k));

                match mod_filter {
                    Some((_, Some(level))) => level >= &metadata.level(),
                    _ if metadata.level() <= min_level => true,
                    _ => false,
                }
            }
            None => metadata.level() <= min_level,
        }
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut mod_path = record.module_path_static().or_else(|| record.module_path()).unwrap_or("<n/a>");

            mod_path = if mod_path == "vanadinite" { "kmain" } else { mod_path.trim_start_matches("vanadinite::") };

            let freq = crate::TIMER_FREQ.load(core::sync::atomic::Ordering::Relaxed);
            let curr_time = crate::csr::time::read();
            let (secs, ms, _) = crate::utils::time_parts(crate::utils::micros(curr_time, freq));

            #[cfg(debug_assertions)]
            {
                let file = record.file_static().or_else(|| record.file()).unwrap_or("<n/a>");

                println!(
                    "[{:>5}.{:<03}] [ {:>5} ] [{} {}:{}] {}",
                    secs,
                    ms,
                    record.level(),
                    mod_path,
                    file,
                    record.line().unwrap_or(0),
                    record.args()
                );
            }

            #[cfg(not(debug_assertions))]
            println!("[{:>5}.{:<03}] [ {:>5} ] [{}] {}", secs, ms, record.level(), mod_path, record.args());
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
