extern crate std;

use crate::*;

static TEST: &[u8] = include_bytes!("../test.dtb");

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        std::eprintln!("[ {:>5} ] [{}] {}", record.level(), record.module_path().unwrap(), record.args());
    }

    fn flush(&self) {}
}

fn init_logging() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = log::set_logger(&StderrLogger);
        log::set_max_level(log::LevelFilter::Trace);
    });
}

#[test]
fn returns_fdt() {
    init_logging();

    assert!(Fdt::new(TEST).is_ok());
}

#[test]
fn finds_root_node() {
    init_logging();

    let fdt = Fdt::new(TEST).unwrap();
    assert!(fdt.find_node("/").is_some(), "couldn't find root node");
}

#[test]
fn finds_root_node_properties() {
    init_logging();

    let fdt = Fdt::new(TEST).unwrap();
    let prop = fdt
        .find_node("/")
        .unwrap()
        .properties()
        .inspect(|p| log::debug!("property: {}", p.name))
        .find(|p| p.name == "compatible" && p.value == b"riscv-virtio\0")
        .is_some();

    assert!(prop);
}

#[test]
fn finds_child_of_root_node() {
    init_logging();

    let fdt = Fdt::new(TEST).unwrap();
    assert!(fdt.find_node("/cpus").is_some(), "couldn't find cpus node");
}
