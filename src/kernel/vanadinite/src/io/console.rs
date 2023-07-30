// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    drivers::{generic::uart16550::Uart16550, sifive::fu540_c000::uart::SifiveUart, CompatibleWith},
    interrupts::isr::register_isr,
    sync::SpinMutex,
    utils::SameHartDeadlockDetection,
};

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

pub struct StaticConsoleDevice(Option<&'static mut dyn ConsoleDevice>);

impl core::fmt::Write for StaticConsoleDevice {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if let Some(console) = &mut self.0 {
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
            inner.init();
        }
    }

    fn read(&self) -> u8 {
        if let Some(inner) = &self.0 {
            return inner.read();
        }

        0
    }

    fn write(&mut self, n: u8) {
        if let Some(inner) = &mut self.0 {
            inner.write(n);
        }
    }
}

unsafe impl Send for StaticConsoleDevice {}
unsafe impl Sync for StaticConsoleDevice {}

pub static CONSOLE: SpinMutex<StaticConsoleDevice, SameHartDeadlockDetection> =
    SpinMutex::new(StaticConsoleDevice(None), SameHartDeadlockDetection::new());

/// # Safety
///
/// 1. The given pointer must be a valid object in memory
/// 2. Be valid for the entirety of runtime
/// 3. Never be used outside of the `CONSOLE`
pub unsafe fn set_raw_console<T: ConsoleDevice>(device: *mut T) {
    let device = &mut *device;
    device.init();

    *CONSOLE.lock() = StaticConsoleDevice(Some(device));
}

pub fn set_console(device: &'static mut dyn ConsoleDevice) {
    device.init();

    *CONSOLE.lock() = StaticConsoleDevice(Some(device));
}

pub enum ConsoleDevices {
    Uart16550,
    SifiveUart,
}

impl ConsoleDevices {
    pub fn from_compatible(compatible: fdt::standard_nodes::Compatible<'_>) -> Option<Self> {
        if compatible.all().any(|s| Uart16550::compatible_with().contains(&s)) {
            Some(ConsoleDevices::Uart16550)
        } else if compatible.all().any(|s| SifiveUart::compatible_with().contains(&s)) {
            Some(ConsoleDevices::SifiveUart)
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// `ptr` must be a valid instance of the device described by the variant in `self`
    pub unsafe fn set_raw_console(&self, ptr: *mut u8) {
        match self {
            ConsoleDevices::Uart16550 => set_raw_console(ptr as *mut Uart16550),
            ConsoleDevices::SifiveUart => set_raw_console(ptr as *mut SifiveUart),
        }
    }

    pub fn register_isr(&self, interrupt_id: usize) {
        match self {
            ConsoleDevices::Uart16550 => register_isr(interrupt_id, console_interrupt),
            ConsoleDevices::SifiveUart => register_isr(interrupt_id, console_interrupt),
        }

        if let Some(plic) = &*crate::interrupts::PLIC.lock() {
            plic.enable_interrupt(crate::platform::current_plic_context(), interrupt_id);
            plic.set_interrupt_priority(interrupt_id, 1);
        }
    }
}

fn console_interrupt(
    _: &crate::drivers::generic::plic::Plic,
    claim: crate::drivers::generic::plic::InterruptClaim<'_>,
    _: usize,
) -> Result<(), &'static str> {
    let _ = CONSOLE.lock().read();
    claim.complete();
    Ok(())
}

pub struct LegacySbiConsoleOut;

impl ConsoleDevice for LegacySbiConsoleOut {
    fn init(&mut self) {}

    fn read(&self) -> u8 {
        sbi::legacy::console_getchar().unwrap_or(0)
    }

    fn write(&mut self, n: u8) {
        sbi::legacy::console_putchar(n)
    }
}
