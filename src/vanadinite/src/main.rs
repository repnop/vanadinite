// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![allow(clippy::match_bool)]
#![allow(incomplete_features)]
#![feature(
    asm,
    naked_functions,
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
mod process;
mod scheduler;
mod sync;
mod thread_local;
mod trap;
mod utils;

mod syscall {
    pub mod exit;
    pub mod print;
    pub mod read_stdin;
}

use {
    arch::csr,
    core::sync::atomic::{AtomicUsize, Ordering},
    drivers::{CompatibleWith, EnableMode},
    interrupts::PLIC,
    mem::{
        kernel_patching,
        paging::{PhysicalAddress, Sv39PageTable, VirtualAddress},
        phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
        phys2virt, virt2phys,
    },
    sync::Mutex,
    utils::Units,
};

extern "C" {
    static stvec_trap_shim: utils::LinkerSymbol;
}

static TIMER_FREQ: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static HART_ID: core::cell::Cell<usize> = core::cell::Cell::new(0);
}

static BLOCK_DEV: Mutex<Option<drivers::virtio::block::BlockDevice>> = Mutex::new(None);

#[no_mangle]
extern "C" fn kmain(hart_id: usize, fdt: *const u8) -> ! {
    // Remove identity mapping after paging initialization
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;
    for address in (kernel_start..kernel_end).step_by(2.mib()) {
        // `kernel_start()` and `kernel_end()` now refer to virtual addresses so
        // we need to patch them back to physical "virtual" addresses to be
        // unmapped
        let patched = VirtualAddress::new(virt2phys(VirtualAddress::new(address)).as_usize());
        unsafe { (&mut *Sv39PageTable::current()).unmap(patched, mem::phys2virt) };
        //(&mut *mem::paging::PAGE_TABLE_ROOT.get()).unmap(patched, mem::phys2virt);
    }

    crate::io::init_logging();

    let fdt = match unsafe { fdt::Fdt::new(fdt) } {
        Some(fdt) => fdt,
        None => crate::arch::exit(crate::arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let current_cpu = fdt.cpus().find(|cpu| cpu.ids().first() == hart_id).unwrap();
    let timebase_frequency = current_cpu.timebase_frequency();
    TIMER_FREQ.store(timebase_frequency, Ordering::Relaxed);

    let mut stdout_interrupts = None;
    let stdout = fdt.chosen().and_then(|n| n.stdout());
    if let Some((node, reg, compatible)) = stdout.and_then(|n| Some((n, n.reg()?.next()?, n.compatible()?))) {
        let stdout_addr = reg.starting_address as *mut u8;

        if let Some(device) = crate::io::ConsoleDevices::from_compatible(compatible) {
            let stdout_phys = PhysicalAddress::from_ptr(stdout_addr);
            let ptr = phys2virt(stdout_phys);

            device.set_console(ptr.as_mut_ptr());

            if let Some(interrupts) = node.interrupts() {
                // Try to get stdout loaded ASAP, so register interrupts later
                // on if there are any
                stdout_interrupts = Some((device, interrupts, ptr));
            }
        }
    }

    let heap_frame_alloc = unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(64) };
    let heap_start = heap_frame_alloc.expect("moar memory").as_phys_address();
    log::debug!("Initing heap at {:#p} (phys {:#p})", mem::phys2virt(heap_start), heap_start);
    unsafe { mem::heap::HEAP_ALLOCATOR.init(mem::phys2virt(heap_start).as_mut_ptr(), 64 * 4.kib()) };

    unsafe { crate::thread_local::init_thread_locals() };
    HART_ID.set(hart_id);

    let model = fdt
        .find_node("/")
        .unwrap()
        .properties()
        .find(|p| p.name == "model")
        .map(|p| core::str::from_utf8(&p.value[..p.value.len() - 1]).unwrap())
        .unwrap();

    let (mem_size, mem_start) = {
        let memory = fdt
            .memory()
            .regions()
            .find(|region| {
                let start = region.starting_address as usize;
                let end = region.starting_address as usize + region.size.unwrap();
                let kstart_phys = mem::virt2phys(VirtualAddress::from_ptr(kernel_patching::kernel_start())).as_usize();
                start <= kstart_phys && kstart_phys <= end
            })
            .unwrap();

        (memory.size.unwrap() / 1024 / 1024, memory.starting_address)
    };

    log::info!("Booted on a {} on hart {}", model, hart_id);
    log::info!("{} MiB of memory starting at {:#p}", mem_size, mem_start);
    log::info!("Timer clock running @ {}Hz", timebase_frequency);
    log::info!("SBI spec version: {:?}", sbi::base::spec_version());
    log::info!("SBI implementor: {:?}", sbi::base::impl_id());
    log::info!("marchid: {:#x}", sbi::base::marchid());
    log::info!("Installing trap handler at {:#p}", unsafe { stvec_trap_shim.as_ptr() });
    csr::stvec::set(unsafe { core::mem::transmute(stvec_trap_shim.as_ptr()) });

    match fdt.all_nodes().find(|node| node.name.starts_with("plic")) {
        Some(ic) => {
            use drivers::generic::plic::Plic;
            let compatible = ic.compatible().unwrap();

            if compatible.all().find(|c| Plic::compatible_with().contains(c)).is_none() {
                panic!("Missing driver for interrupt controller!");
            }

            let reg = ic.reg().unwrap().next().unwrap();
            let ic_phys = PhysicalAddress::from_ptr(reg.starting_address);
            let ic_virt = phys2virt(ic_phys);

            let plic = unsafe { &*ic_virt.as_ptr().cast::<Plic>() };

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

    drivers::Plic::context_threshold(&*PLIC.lock(), drivers::EnableMode::Local, 0x00);

    for child in fdt.find_all_nodes("/soc/virtio_mmio") {
        use drivers::virtio::mmio::{
            block::VirtIoBlockDevice,
            common::{DeviceType, VirtIoHeader},
        };
        let reg = child.reg().unwrap().next().unwrap();

        let virtio_mmio_phys = PhysicalAddress::from_ptr(reg.starting_address);
        let virtio_mmio_virt = phys2virt(virtio_mmio_phys);

        let device: &'static VirtIoHeader = unsafe { &*(virtio_mmio_virt.as_ptr().cast()) };

        if let Some(DeviceType::BlockDevice) = device.device_type() {
            let block_device = unsafe { &*(device as *const _ as *const VirtIoBlockDevice) };

            *BLOCK_DEV.lock() = Some(drivers::virtio::block::BlockDevice::new(block_device).unwrap());

            let plic = &*PLIC.lock();
            for interrupt in child.interrupts().unwrap() {
                drivers::Plic::enable_interrupt(plic, EnableMode::Local, interrupt);
                drivers::Plic::interrupt_priority(plic, interrupt, 1);
                interrupts::isr::register_isr::<drivers::virtio::block::BlockDevice>(interrupt, 0);
            }
        }
    }

    unsafe {
        *process::THREAD_CONTROL_BLOCK.get() = process::ThreadControlBlock {
            kernel_stack: mem::alloc_kernel_stack(8.kib()),
            kernel_thread_local: thread_local::tp(),
            saved_sp: 0,
            saved_tp: 0,
            kernel_stack_size: 8.kib(),
            current_process: None,
        };

        arch::csr::sscratch::write(process::THREAD_CONTROL_BLOCK.get() as usize);
    }

    arch::csr::sstatus::set_fs(arch::csr::sstatus::FloatingPointStatus::Initial);
    arch::csr::sie::enable();
    //arch::csr::sstatus::enable_interrupts();
    //
    //loop {
    //    unsafe { asm!("wfi") };
    //    if let Some(c) = io::INPUT_QUEUE.pop() {
    //        print!("{}", c as char);
    //    }
    //}

    //pub static TEMPLATE: &[u8] =
    //    include_bytes!("../../../userspace/template/target/riscv64gc-unknown-none-elf/release/template");
    //pub static HAX: &[u8] = include_bytes!("../../../userspace/hax/target/riscv64gc-unknown-none-elf/release/hax");
    pub static SHELL: &[u8] =
        include_bytes!("../../../userspace/shell/target/riscv64gc-unknown-none-elf/release/shell");

    //scheduler::Scheduler::push(&*scheduler::SCHEDULER, process::Process::load(&elf64::Elf::new(TEMPLATE).unwrap()));
    //scheduler::Scheduler::push(&*scheduler::SCHEDULER, process::Process::load(&elf64::Elf::new(HAX).unwrap()));
    scheduler::Scheduler::push(&*scheduler::SCHEDULER, process::Process::load(&elf64::Elf::new(SHELL).unwrap()));

    log::info!("Scheduling init process!");

    scheduler::Scheduler::schedule(&*scheduler::SCHEDULER);

    //arch::exit(arch::ExitStatus::Ok)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    arch::exit(arch::ExitStatus::Error(info))
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    arch::exit(arch::ExitStatus::Error(&"aborted"))
}

#[alloc_error_handler]
fn alloc_error_handler(_: alloc::alloc::Layout) -> ! {
    panic!("out of memory")
}
