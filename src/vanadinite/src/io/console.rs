// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    drivers::{generic::uart16550::Uart16550, sifive::fu540_c000::uart::SifiveUart, CompatibleWith, EnableMode, Plic},
    sync::{Mutex, RwLock},
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
    SifiveUart,
}

impl ConsoleDevices {
    pub fn from_compatible(ptr: *mut u8, compatible: fdt::Compatible<'_>) -> Option<Self> {
        if compatible.all().any(|s| Uart16550::compatible_with().contains(&s)) {
            Some(ConsoleDevices::Uart16550)
        } else if compatible.all().any(|s| SifiveUart::compatible_with().contains(&s)) {
            Some(ConsoleDevices::SifiveUart)
        } else {
            None
        }
    }

    pub fn set_console(&self, ptr: *mut u8) {
        match self {
            ConsoleDevices::Uart16550 => unsafe { set_console(ptr as *mut Uart16550) },
            ConsoleDevices::SifiveUart => unsafe { set_console(ptr as *mut SifiveUart) },
        }
    }

    pub fn register_isr(&self, interrupt_id: usize, private: usize) {
        match self {
            ConsoleDevices::Uart16550 => crate::interrupts::isr::register_isr::<Uart16550>(interrupt_id, private),
            ConsoleDevices::SifiveUart => crate::interrupts::isr::register_isr::<SifiveUart>(interrupt_id, private),
        }

        let plic = crate::interrupts::PLIC.lock();
        plic.enable_interrupt(EnableMode::Local, interrupt_id);
        plic.interrupt_priority(interrupt_id, 1);
    }
}
