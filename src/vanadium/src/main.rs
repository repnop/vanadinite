// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(asm, naked_functions, fn_align)]
#![no_std]
#![no_main]

pub mod csr;
pub mod sbi;

#[naked]
#[no_mangle]
#[rustfmt::skip]
#[allow(named_asm_labels)]
#[link_section = ".boot.entry"]
/// # Safety
/// no2
pub unsafe extern "C" fn _entry(_fdt: *const u8) -> ! {
    asm!("
        # Disable interrupts
        csrci mstatus, 3

        csrr t0, misa

        # Get S extension bit index
        li t1, 'S' - 'A'

        # Check S extension bit
        # If not set, we're likely on a monitor core
        srl t0, t0, t1
        andi t0, t0, 1
        beqz t0, .monitor_core

        # We're not a monitor core if we're here
        # So let the games begin!

        # Get the address of the AtomicU32
        lla t0, HART_LOTTERY
        li t1, 1

        lr.w t2, (t0)
        bnez t2, .lottery_losers
        sc.w t2, t1, (t0)
        bnez t2, .lottery_losers
        
        # The winner needs to initialize stuff
        .lottery_winner:
            .option push
            .option norelax
            lla gp, __global_pointer$
            .option pop

            lla t0, __bss_start
            lla t1, __bss_end
    
            # We must clear the .bss section here since its assumed to be zero on first access
            clear_bss:
                beq t0, t1, done_clear_bss
                sd zero, (t0)
                addi t0, t0, 8
                j clear_bss
            done_clear_bss:

            lla sp, __tmp_stack_top
            j main

        .lottery_losers:
            # Re-enable interrupts for when we're to be woken up
            csrsi mstatus, 3
            1:  wfi
                j 1b

        .monitor_core:
            wfi
            j .monitor_core

        .section .data
        .balign 4
        HART_LOTTERY: .4byte 0
    
    ", options(noreturn));
}

#[no_mangle]
pub extern "C" fn main(_fdt: *const u8) -> ! {
    log::set_logger(&TempLogger).expect("failed to init logging");
    log::set_max_level(log::LevelFilter::Trace);

    log::info!("Hello from hart {}", csr::mhartid::read());

    #[allow(clippy::empty_loop)]
    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("PANIC: {}", info);
    loop {}
}

struct TempLogger;
impl log::Log for TempLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            crate::println!("{}", record.args());
        }
    }

    fn flush(&self) {}
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => ($crate::print!("{}\r\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    <VirtUart as core::fmt::Write>::write_fmt(&mut VirtUart, args).unwrap();
}

struct VirtUart;
impl core::fmt::Write for VirtUart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            #[cfg(feature = "platform.virt")]
            unsafe { *(0x10000000 as *mut u8) = b };

            #[cfg(feature = "platform.sifive_u")]
            unsafe { *(0x10010000 as *mut u32) = b as u32 };
        }

        Ok(())
    }
}
