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
use log::info;
use mem::{
    kernel_patching,
    paging::{PhysicalAddress, VirtualAddress, PAGE_TABLE_MANAGER},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
};

use core::sync::atomic::{AtomicUsize, Ordering};

const TWO_MEBS: usize = 2 * 1024 * 1024;

extern "C" {
    static stvec_trap_shim: utils::LinkerSymbol;
}

static TIMER_FREQ: AtomicUsize = AtomicUsize::new(0);

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
        let patched = VirtualAddress::new(mem::virt2phys(VirtualAddress::new(address)).as_usize());
        page_manager.unmap_with_translation(patched, mem::phys2virt);
        //(&mut *mem::paging::PAGE_TABLE_ROOT.get()).unmap(patched, mem::phys2virt);
    }

    crate::io::init_logging();

    let fdt = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => crate::arch::exit(crate::arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let mut stdout_interrupts = None;
    let stdout = fdt.chosen().and_then(|n| n.stdout());
    if let Some((node, reg, compatible)) = stdout.and_then(|n| Some((n, n.reg()?.next()?, n.compatible()?))) {
        let stdout_addr = reg.starting_address as *mut u8;
        let stdout_size = utils::round_up_to_next(reg.size.unwrap(), 4096);

        if let Some(device) = crate::io::ConsoleDevices::from_compatible(stdout_addr, compatible) {
            let stdout_phys = PhysicalAddress::from_ptr(stdout_addr);
            let ptr = page_manager.map_mmio(stdout_phys, stdout_size);

            device.set_console(ptr.as_mut_ptr());

            if let Some(interrupts) = node.interrupts() {
                // Try to get stdout loaded ASAP, so register interrupts later
                // on if there are any
                stdout_interrupts = Some((device, interrupts, ptr));
            }
        }
    }

    match fdt.all_nodes().find(|node| node.name.starts_with("plic")) {
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

            log::info!("Registering PLIC @ {:#p}", ic_virt);
            interrupts::register_plic(plic as &'static dyn drivers::Plic);
        }
        None => panic!("Can't find interrupt controller!"),
    }

    if let Some((device, interrupts, ptr)) = stdout_interrupts {
        for interrupt in interrupts {
            device.register_isr(interrupt, ptr.as_usize());
        }
    }

    drivers::Plic::context_threshold(&*interrupts::PLIC.lock(), drivers::EnableMode::Local, 0x00);

    let heap_start = PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(60).expect("moar memory").as_phys_address();
    log::info!("Initing heap at {:#p} (phys {:#p})", mem::phys2virt(heap_start), heap_start);
    mem::heap::HEAP_ALLOCATOR.init(mem::phys2virt(heap_start).as_mut_ptr(), 64 * utils::Units::kib(4));

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

    let current_cpu = fdt.cpus().find(|cpu| cpu.ids().first() == hart_id).unwrap();
    let timebase_frequency = current_cpu.timebase_frequency();
    TIMER_FREQ.store(timebase_frequency, Ordering::Relaxed);

    println!("timebase frequency: {}Hz", timebase_frequency);

    //#[cfg(feature = "sifive_u")]
    asm::pause();

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
