#![feature(asm, prelude_import, no_core)]
#![no_std]

extern crate rt0;

pub mod io;
pub mod prelude;
pub mod syscalls;

#[prelude_import]
pub use prelude::v1::*;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[derive(Debug)]
#[repr(C)]
pub enum KResult<T, E> {
    Ok(T),
    Err(E),
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let _ = io::Stdout.write_fmt(args);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    syscalls::exit()
}
