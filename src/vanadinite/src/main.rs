// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![allow(clippy::match_bool)]
#![allow(incomplete_features)]
#![feature(
    asm,
    naked_functions,
    global_asm,
    alloc_error_handler,
    raw_ref_op,
    const_generics,
    const_in_array_repeat_expressions,
    thread_local,
    maybe_uninit_ref,
    const_fn_fn_ptr_basics,
    const_fn
)]
#![no_std]
#![no_main]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("vanadinite assumes a 64-bit pointer size, cannot compile on non-64 bit systems");

extern crate alloc;

mod arch;
mod asm;
mod boot;
mod drivers;
mod interrupts;
mod io;
mod mem;
mod sync;
mod thread_local;
mod trap;
mod utils;

use arch::csr;
use drivers::CompatibleWith;
use mem::{
    kernel_patching,
    paging::{PhysicalAddress, VirtualAddress, PAGE_TABLE_MANAGER},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
};

const TWO_MEBS: usize = 2 * 1024 * 1024;

extern "C" {
    static stvec_trap_shim: utils::LinkerSymbol;
}

thread_local! {
    static HART_ID: core::cell::Cell<usize> = core::cell::Cell::new(0);
}

#[no_mangle]
unsafe extern "C" fn kmain(hart_id: usize, fdt: *const u8) -> ! {
    crate::thread_local::init_thread_locals();
    HART_ID.set(hart_id);

    let mut page_manager = PAGE_TABLE_MANAGER.lock();
    // Remove identity mapping after paging initialization
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;
    for address in (kernel_start..kernel_end).step_by(TWO_MEBS) {
        // `kernel_start()` and `kernel_end()` now refer to virtual addresses so
        // we need to patch them back to physical "virtual" addresses to be
        // unmapped
        let patched = VirtualAddress::new(kernel_patching::virt2phys(VirtualAddress::new(address)).as_usize());
        page_manager.unmap_with_translation(patched, kernel_patching::phys2virt);
        //(&mut *mem::paging::PAGE_TABLE_ROOT.get()).unmap(patched, kernel_patching::phys2virt);
    }

    crate::io::init_logging();

    let fdt = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => crate::arch::exit(crate::arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    match fdt.find_node("/soc/interrupt-controller") {
        Some(ic) => {
            use drivers::generic::plic::Plic;
            let compatible = ic.compatible().unwrap();

            if compatible.all().find(|c| Plic::compatible_with().contains(c)).is_none() {
                panic!("Missing driver for interrupt controller!");
            }

            let reg = ic.reg().unwrap().next().unwrap();
            let ic_phys = PhysicalAddress::from_ptr(reg.starting_address);
            let ic_virt = page_manager.map_mmio(ic_phys, reg.size.unwrap());

            let plic = &*ic_virt.as_ptr().cast::<Plic>();
            // Plic::init(plic);

            log::info!("Registering PLIC @ {:#p}", ic_virt);
            interrupts::register_plic(plic as &'static dyn drivers::Plic);
        }
        None => panic!("Can't find interrupt controller!"),
    }

    drivers::Plic::context_threshold(&*interrupts::PLIC.lock(), drivers::EnableMode::Local, 0x00);

    let stdout = fdt.chosen().and_then(|n| n.stdout());
    if let Some((node, reg, compatible)) = stdout.and_then(|n| Some((n, n.reg()?.next()?, n.compatible()?))) {
        let stdout_addr = reg.starting_address as *mut u8;
        let stdout_size = utils::round_up_to_next(reg.size.unwrap(), 4096);

        if let Some(device) = crate::io::ConsoleDevices::from_compatible(stdout_addr, compatible) {
            let stdout_phys = PhysicalAddress::from_ptr(stdout_addr);
            let ptr = page_manager.map_mmio(stdout_phys, stdout_size);

            device.set_console(ptr.as_mut_ptr());

            if let Some(interrupts) = node.interrupts() {
                for interrupt in interrupts {
                    log::info!("interrupt id: {}", interrupt);
                    device.register_isr(interrupt, ptr.as_usize());
                }
            }
        }
    }

    //drivers::Plic::context_threshold(&*interrupts::PLIC.lock(), drivers::EnableMode::Local, 0x00);

    log::info!(
        "Booted on a {} on hart {}",
        fdt.find_node("/")
            .unwrap()
            .properties()
            .find(|p| p.name == "model")
            .map(|p| core::str::from_utf8(&p.value[..p.value.len() - 1]).unwrap())
            .unwrap(),
        hart_id
    );
    log::info!("SBI spec version: {:?}", sbi::base::spec_version());
    log::info!("SBI implementor: {:?}", sbi::base::impl_id());
    log::info!("marchid: {:#x}", sbi::base::marchid());
    log::info!("Installing trap handler at {:#p}", stvec_trap_shim.as_ptr());
    csr::stvec::set(core::mem::transmute(stvec_trap_shim.as_ptr()));

    let tp: usize;
    asm!("mv {}, tp", out(reg) tp);
    println!("tp: {:#p}", tp as *mut u8);
    println!("hart_id: {}", HART_ID.get());

    for child in fdt.find_all_nodes("/virtio_mmio") {
        use drivers::virtio::mmio::{
            block::{FeatureBits, VirtIoBlockDevice},
            common::{DeviceType, VirtIoHeader},
        };
        let reg = child.reg().unwrap().next().unwrap();

        let virtio_mmio_phys = PhysicalAddress::from_ptr(reg.starting_address);
        let virtio_mmio_virt = page_manager.map_mmio(virtio_mmio_phys, reg.size.unwrap());

        let device: &'static VirtIoHeader = &*(virtio_mmio_virt.as_ptr().cast());

        if let Some(DeviceType::BlockDevice) = device.device_type() {
            let block_device: &'static VirtIoBlockDevice = &*(device as *const _ as *const VirtIoBlockDevice);

            println!("{:?}: {:?}", FeatureBits::SizeMax, block_device.features() & FeatureBits::SizeMax);
            println!("{:?}: {:?}", FeatureBits::Geometry, block_device.features() & FeatureBits::Geometry);
            println!("{:?}: {:?}", FeatureBits::ReadOnly, block_device.features() & FeatureBits::ReadOnly);
            println!("{:?}: {:?}", FeatureBits::BlockSize, block_device.features() & FeatureBits::BlockSize);
            println!("{:?}: {:?}", FeatureBits::WriteZeroes, block_device.features() & FeatureBits::WriteZeroes);
        }
    }

    // TIMEBASE-FREQUENCY!!!!!!!!!!!!!!!!!!!

    arch::csr::sstatus::enable_interrupts();
    arch::csr::sie::enable();

    println!("{:?}", alloc::boxed::Box::new(5i32));

    println!("{:?}", alloc::vec![1u64; 512]);

    arch::exit(arch::ExitStatus::Ok)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    arch::exit(arch::ExitStatus::Error(info))
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!("we've aborted")
}

#[alloc_error_handler]
fn alloc_error_handler(_: alloc::alloc::Layout) -> ! {
    panic!()
}
