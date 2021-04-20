// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::sync::RwLock;
use alloc::{collections::BTreeMap, string::String};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use log::LevelFilter;

static HART_FILTER: AtomicUsize = AtomicUsize::new(usize::max_value());
static LOG_FILTER: RwLock<Option<BTreeMap<String, Option<LevelFilter>>>> = RwLock::new(None);
static LOG_LEVEL: AtomicUsize = AtomicUsize::new(LevelFilter::Info as usize);
pub static USE_COLOR: AtomicBool = AtomicBool::new(true);

pub fn parse_log_filter(filter: Option<&str>) {
    if let Some(filter) = filter {
        let mut map = BTreeMap::new();
        for part in filter.split(',') {
            let mut parts = part.split('=');
            let name = parts.next().unwrap();

            match name {
                "ignore-harts" => match parts.next() {
                    Some(list) => {
                        let mut mask = 0;
                        for n in list.split(',').filter_map(|n| n.parse::<usize>().ok()) {
                            mask |= 1 << n;
                        }

                        HART_FILTER.fetch_xor(mask, Ordering::Relaxed);
                    }
                    None => log::warn!("Missing hart list for `ignore-harts`"),
                },
                _ => {
                    if let Some(level) = level_from_str(name) {
                        set_max_level(level);
                        continue;
                    }
                }
            }

            let level = match parts.next() {
                Some(level) => match level_from_str(level) {
                    Some(level) => Some(level),
                    None => {
                        log::warn!("Bad level filter: '{}', skipping", level);
                        continue;
                    }
                },
                None => None,
            };

            map.insert(String::from(name), level);
        }

        *LOG_FILTER.write() = Some(map);
    }
}

fn level_from_str(level: &str) -> Option<LevelFilter> {
    match level {
        "off" => Some(LevelFilter::Off),
        "trace" => Some(LevelFilter::Trace),
        "debug" => Some(LevelFilter::Debug),
        "info" => Some(LevelFilter::Info),
        "warn" => Some(LevelFilter::Warn),
        "error" => Some(LevelFilter::Error),
        _ => None,
    }
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let hart_id = crate::HART_ID.get();

        if HART_FILTER.load(Ordering::Relaxed) & (1 << hart_id) == 0 {
            return false;
        }

        let max_level = max_level();

        let mut mod_path = metadata.target();
        mod_path = if mod_path == "vanadinite" { "kmain" } else { mod_path.trim_start_matches("vanadinite::") };

        let filter = LOG_FILTER.read();
        match &*filter {
            Some(filters) => {
                let mod_filter = filters.iter().find(|(k, _)| mod_path.starts_with(*k));

                match mod_filter {
                    Some((_, Some(level))) => metadata.level() <= *level,
                    _ if metadata.level() <= max_level => true,
                    _ => false,
                }
            }
            None => metadata.level() <= max_level,
        }
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut mod_path = record.module_path_static().or_else(|| record.module_path()).unwrap_or("<n/a>");

            mod_path = if mod_path == "vanadinite" { "kmain" } else { mod_path.trim_start_matches("vanadinite::") };

            let freq = crate::TIMER_FREQ.load(core::sync::atomic::Ordering::Relaxed);
            let curr_time = crate::csr::time::read();
            let (secs, ms, _) = crate::utils::time_parts(crate::utils::micros(curr_time, freq));

            let color = match record.level() {
                log::Level::Trace => crate::io::terminal::WHITE,
                log::Level::Debug => crate::io::terminal::GREEN,
                log::Level::Info => crate::io::terminal::BLUE,
                log::Level::Warn => crate::io::terminal::YELLOW,
                log::Level::Error => crate::io::terminal::RED,
            };

            let clear = crate::io::terminal::CLEAR;

            crate::println!(
                "[{:>5}.{:<03}] [ {}{:>5}{} ] [HART {}] [{}] {}",
                secs,
                ms,
                color,
                record.level(),
                clear,
                crate::HART_ID.get(),
                mod_path,
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

pub fn init_logging() {
    log::set_logger(&Logger).expect("failed to init logging");
    log::set_max_level(log::LevelFilter::Trace);
}

fn max_level() -> LevelFilter {
    unsafe { core::mem::transmute(LOG_LEVEL.load(Ordering::Relaxed)) }
}

fn set_max_level(filter: LevelFilter) {
    LOG_LEVEL.store(filter as usize, Ordering::Relaxed)
}
