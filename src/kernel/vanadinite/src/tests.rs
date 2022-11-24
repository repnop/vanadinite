// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

// For some reason there's a false positive down below, check to see if this can
// be removed in the future
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::{
    cpu_local, csr,
    drivers::{generic::plic::Plic, CompatibleWith},
    interrupts,
    mem::{self, paging::PhysicalAddress, phys2virt},
    platform::{self, ExitStatus},
    task, trap,
    utils::Units,
    HART_ID, N_CPUS, TIMER_FREQ,
};
use alloc::boxed::Box;
use core::sync::atomic::Ordering;
use fdt::Fdt;

#[no_mangle]
#[repr(align(4))]
pub extern "C" fn ktest(hart_id: usize, fdt: *const u8) -> ! {
    csr::stvec::set(trap::stvec_trap_shim);

    unsafe { cpu_local::init_thread_locals() };
    HART_ID.set(hart_id);

    crate::io::logging::init_logging();

    mem::heap::HEAP_ALLOCATOR.init(64.mib());

    let fdt: Fdt<'static> = match unsafe { Fdt::from_ptr(fdt) } {
        Ok(fdt) => fdt,
        Err(e) => crate::platform::exit(crate::platform::ExitStatus::Error(&e)),
    };

    let current_cpu = fdt.cpus().find(|cpu| cpu.ids().first() == hart_id).unwrap();
    let timebase_frequency = current_cpu.timebase_frequency();
    TIMER_FREQ.store(timebase_frequency as u64, Ordering::Relaxed);

    let stdout = fdt.chosen().stdout();
    if let Some((_, reg, compatible)) = stdout.and_then(|n| Some((n, n.reg()?.next()?, n.compatible()?))) {
        let stdout_addr = reg.starting_address as *mut u8;

        if let Some(device) = crate::io::ConsoleDevices::from_compatible(compatible) {
            let stdout_phys = PhysicalAddress::from_ptr(stdout_addr);
            let ptr = phys2virt(stdout_phys);

            unsafe { device.set_raw_console(ptr.as_mut_ptr()) };
        }
    }

    let n_cpus = fdt.cpus().count();
    N_CPUS.store(n_cpus, Ordering::Release);

    if let Some(ic) = fdt.find_compatible(Plic::compatible_with()) {
        let reg = ic.reg().unwrap().next().unwrap();
        let ic_phys = PhysicalAddress::from_ptr(reg.starting_address);
        let ic_virt = phys2virt(ic_phys);

        // Number of interrupts available
        let ndevs = ic
            .properties()
            .find(|p| p.name == "riscv,ndev")
            .and_then(|p| p.as_usize())
            .expect("missing number of interrupts");

        // Find harts which have S-mode available
        let contexts = fdt
            .cpus()
            .filter(|cpu| {
                cpu.properties()
                    .find(|p| p.name == "riscv,isa")
                    .and_then(|p| p.as_str()?.chars().find(|c| *c == 's'))
                    .is_some()
            })
            .map(|cpu| platform::plic_context_for(cpu.ids().first()));

        let plic = unsafe { &*ic_virt.as_ptr().cast::<Plic>() };

        plic.init(ndevs, contexts);
        plic.set_context_threshold(platform::current_plic_context(), 0);

        log::debug!("Registering PLIC @ {:#p}", ic_virt);
        interrupts::register_plic(plic);
    }

    #[cfg(test)]
    crate::test_main();

    platform::exit(ExitStatus::Ok)
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    crate::println!("\nRunning {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[test]
fn it_works() {
    assert!(true);
}

#[cfg_attr(test, panic_handler)]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::println!("{}failed{}: {}", crate::io::terminal::RED, crate::io::terminal::CLEAR, info);

    platform::exit(ExitStatus::Error(&"test failed"))
}
