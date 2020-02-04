#[macro_use]
pub mod uart;

pub fn init_uart_logger() {
    log::set_logger(&uart::UartLogger).unwrap();
    #[cfg(debug_assertions)]
    log::set_max_level(log::LevelFilter::Trace);
    #[cfg(not(debug_assertions))]
    log::set_max_level(log::LevelFilter::Info);
}
