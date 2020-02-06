#[macro_use]
pub mod uart;

pub fn init_uart_logger() {
    log::set_logger(&uart::UartLogger).unwrap();
    #[cfg(debug_assertions)]
    log::set_max_level(log::LevelFilter::Trace);
    #[cfg(not(debug_assertions))]
    log::set_max_level(log::LevelFilter::Info);
}

#[repr(u64)]
pub enum Finisher {
    Fail = 0x3333,
    Pass = 0x5555,
    Reset = 0x7777,
}

/// So right about now is where I wish QEMU was better documented. Searching
/// through the code on Github for about 45 minutes resulted in the following
/// discovery:
///
/// To exit QEMU from inside it, we have to write to a special memory location
/// with a certain format. This is know for x86{_64} and ARM/AArch64 but I
/// couldn't find any resources on it for RISC-V.
///
/// It turns out that the `virt` board uses the same MMIO debug stuff as the
/// SiFive board, so you can subsequently find the information in that
/// header/implementation file pair at time of writing:
///
/// https://github.com/qemu/qemu/blob/master/include/hw/riscv/sifive_test.h
/// https://github.com/qemu/qemu/blob/master/hw/riscv/sifive_test.c
///
/// Which is created here for the `virt` board:
///
/// https://github.com/qemu/qemu/blob/master/hw/riscv/virt.c#L566
///
/// So with all of this information we can gather that to exit QEMU we must:
///
///     1. Construct a 64-bit value to write
///         1a. The bottom 16-bits are the status code
///         1b. The next set of 16-bits are the exit code (this is ignored for Finisher::Pass which is always 0)
///     2. Write this value to VIRT_TEST (0x100000) + 0x000000
///     3. Pray we've actually exited, otherwise panic
///
pub fn exit(finisher: Finisher, error_code: u16) -> ! {
    const VIRT_TEST: *mut u64 = 0x10_0000 as *mut u64;

    log::info!("Exiting QEMU with error code: {}", error_code);
    let exit_value = finisher as u64 | ((error_code as u64) << 16);

    unsafe {
        core::ptr::write_volatile(VIRT_TEST, exit_value);
    }

    unreachable!()
}
