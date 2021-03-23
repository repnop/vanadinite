// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![allow(clippy::match_bool, clippy::identity_op)]
#![allow(incomplete_features)]
#![feature(
    alloc_error_handler,
    asm,
    const_fn_fn_ptr_basics,
    const_fn,
    const_generics,
    inline_const,
    maybe_uninit_ref,
    naked_functions,
    raw_ref_op,
    thread_local
)]
#![no_std]
#![no_main]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("vanadinite assumes a 64-bit pointer size, cannot compile on non-64 bit systems");

extern crate alloc;

pub mod asm;
pub mod boot;
pub mod cpu_local;
pub mod csr;
pub mod drivers;
pub mod interrupts;
pub mod io;
pub mod mem;
pub mod platform;
pub mod process;
pub mod scheduler;
pub mod sync;
pub mod trap;
pub mod utils;

mod syscall {
    pub mod exit;
    pub mod print;
    pub mod read_stdin;
}

use {
    core::sync::atomic::{AtomicUsize, Ordering},
    drivers::{generic::plic::Plic, CompatibleWith},
    interrupts::PLIC,
    mem::{
        kernel_patching,
        paging::{PhysicalAddress, VirtualAddress},
        phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
        phys2virt,
    },
    sync::Mutex,
    utils::Units,
};

use fdt::Fdt;
use mem::kernel_patching::kernel_section_v2p;
use sbi::hart_state_management::hart_start;
pub use vanadinite_macros::{debug, error, info, trace, warn};

static TIMER_FREQ: AtomicUsize = AtomicUsize::new(0);
static INIT_FS: &[u8] = include_bytes!("../../../initfs.tar");
static BLOCK_DEV: Mutex<Option<drivers::virtio::block::BlockDevice>> = Mutex::new(None);

cpu_local! {
    static HART_ID: core::cell::Cell<usize> = core::cell::Cell::new(0);
}

#[no_mangle]
extern "C" fn kmain(hart_id: usize, fdt: *const u8) -> ! {
    unsafe { asm!(".align 4") };

    let heap_frame_alloc = unsafe { PHYSICAL_MEMORY_ALLOCATOR.lock().alloc_contiguous(64) };
    let heap_start = heap_frame_alloc.expect("moar memory").as_phys_address();
    unsafe { mem::heap::HEAP_ALLOCATOR.init(mem::phys2virt(heap_start).as_mut_ptr(), 64 * 4.kib()) };

    unsafe { crate::cpu_local::init_thread_locals() };
    HART_ID.set(hart_id);

    crate::io::logging::init_logging();

    let fdt: Fdt<'static> = match unsafe { Fdt::from_ptr(fdt) } {
        Ok(fdt) => fdt,
        Err(e) => crate::platform::exit(crate::platform::ExitStatus::Error(&e)),
    };

    let current_cpu = fdt.cpus().find(|cpu| cpu.ids().first() == hart_id).unwrap();
    let timebase_frequency = current_cpu.timebase_frequency();
    TIMER_FREQ.store(timebase_frequency, Ordering::Relaxed);

    let mut stdout_interrupts = None;
    let stdout = fdt.chosen().stdout();
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

    let mut init_path = "init";
    if let Some(args) = fdt.chosen().bootargs() {
        let split_args = args.split(' ').map(|s| {
            let mut parts = s.splitn(2, '=');
            (parts.next().unwrap(), parts.next())
        });

        for (option, value) in split_args {
            match option {
                "log-filter" => io::logging::parse_log_filter(value),
                "init" => match value {
                    Some(path) => init_path = path,
                    None => log::warn!("No path provided for init process! Defaulting to `init`"),
                },
                "no-color" | "no-colour" => io::logging::USE_COLOR.store(false, Ordering::Relaxed),
                "" => {}
                _ => log::warn!("Unknown kernel argument: `{}`", option),
            }
        }
    }

    let model = fdt.root().property("model").and_then(|p| p.as_str()).unwrap();

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

    let (impl_major, impl_minor) = {
        let version = sbi::base::impl_version();
        // This is how OpenSBI encodes their version, hopefully will be the same
        // between others
        (version >> 16, version & 0xFFFF)
    };

    let (spec_major, spec_minor) = {
        let version = sbi::base::spec_version();
        (version.major, version.minor)
    };

    let n_cpus = fdt.cpus().count();

    info!("vanadinite version {#brightgreen}", env!("CARGO_PKG_VERSION"));
    info!(blue, "=== Machine Info ===");
    info!(" Device Model: {}", model);
    info!(" Total CPUs: {}", n_cpus);
    info!(" RAM: {} MiB @ {:#X}", mem_size, mem_start as usize);
    info!(" Timer Clock: {}Hz", timebase_frequency);
    info!(blue, "=== SBI Implementation ===");
    info!(" Implementor: {:?} (version: {#green'{}.{}})", sbi::base::impl_id(), impl_major, impl_minor);
    info!(" Spec Version: {#green'{}.{}}", spec_major, spec_minor);

    debug!("Installing trap handler at {:#p}", trap::stvec_trap_shim as *const u8);
    csr::stvec::set(trap::stvec_trap_shim);

    match fdt.find_compatible(Plic::compatible_with()) {
        Some(ic) => {
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

            debug!("Registering PLIC @ {:#p}", ic_virt);
            interrupts::register_plic(plic);
        }
        None => panic!("Can't find PLIC!"),
    }

    if let Some((device, interrupts, ptr)) = stdout_interrupts {
        for interrupt in interrupts {
            device.register_isr(interrupt, ptr.as_usize());
        }
    }

    PLIC.lock().set_context_threshold(platform::current_plic_context(), 0);

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

            let plic = PLIC.lock();
            for interrupt in child.interrupts().unwrap() {
                plic.enable_interrupt(platform::current_plic_context(), interrupt);
                plic.set_interrupt_priority(interrupt, 1);
                interrupts::isr::register_isr::<drivers::virtio::block::BlockDevice>(interrupt, 0);
            }
        }
    }

    let other_hart_boot_phys = unsafe { kernel_section_v2p(VirtualAddress::from_ptr(other_hart_boot as *const u8)) };

    for cpu in fdt.cpus().filter(|cpu| cpu.ids().first() != hart_id) {
        let hart_id = cpu.ids().first();
        let hart_sp = mem::alloc_kernel_stack(8.kib()) as usize;

        if let Err(e) = hart_start(hart_id, other_hart_boot_phys.as_usize(), hart_sp) {
            error!(red, "Failed to start hart {}: {:?}", hart_id, e);
        }
    }

    unsafe {
        *process::THREAD_CONTROL_BLOCK.get() = process::ThreadControlBlock {
            kernel_stack: mem::alloc_kernel_stack(8.kib()),
            kernel_thread_local: cpu_local::tp(),
            saved_sp: 0,
            saved_tp: 0,
            kernel_stack_size: 8.kib(),
            current_process: None,
        };

        csr::sscratch::write(process::THREAD_CONTROL_BLOCK.get() as usize);
    }

    csr::sstatus::set_fs(csr::sstatus::FloatingPointStatus::Initial);
    csr::sie::enable();

    let tar = tar::Archive::new(INIT_FS).unwrap();

    scheduler::Scheduler::push(process::Process::load(
        &elf64::Elf::new(tar.file(init_path).unwrap().contents).unwrap(),
    ));

    info!(brightgreen, "Scheduling init process!");

    scheduler::Scheduler::schedule()
}

extern "C" fn kalt(hart_id: usize) -> ! {
    unsafe { asm!(".align 4") };
    unsafe { crate::cpu_local::init_thread_locals() };
    HART_ID.set(hart_id);

    info!(brightgreen, "Hello from hart ID: {}", hart_id);

    loop {
        unsafe { asm!("wfi") };
    }
}

#[naked]
#[no_mangle]
unsafe extern "C" fn other_hart_boot() -> ! {
    #[rustfmt::skip]
    asm!(
        "
            # We start here with only two registers in a defined state:
            #  a0: hart id
            #  a1: stack pointer (virtual)
            #
            # We need to initialize the following things:
            #   satp: to the physical address of the root page table
            #  stvec: to the virtual address we'll trap-trick to
            #     sp: to the stack region we receive in a1
            #     gp: to the virtual GP pointer
            

            # Translate phys `__global_pointer$` addr to virtual
            lla t0, __global_pointer$

            lla t1, PAGE_OFFSET_VALUE
            ld t1, (t1)

            lla t2, PAGE_OFFSET

            sub t0, t0, t2
            add t0, t0, t1

            # Set up gp and sp
            mv gp, t0
            mv sp, a1

            # Translate phys `kalt` addr to virtual
            lla t0, {}
            sub t0, t0, t2
            add t0, t0, t1
            csrw stvec, t0

            # Load root page table physical address
            lla t0, {}
            srli t0, t0, 12
            li t1, 8
            slli t1, t1, 60
            or t0, t1, t0

            csrw satp, t0
            sfence.vma
            nop             # We fault here and fall into `kalt`
        ",
        sym kalt,
        sym boot::early_paging::PAGE_TABLE_ROOT,
        options(noreturn),
    );
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("{}", info);
    platform::exit(platform::ExitStatus::Error(info))
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    platform::exit(platform::ExitStatus::Error(&"aborted"))
}

#[alloc_error_handler]
fn alloc_error_handler(_: alloc::alloc::Layout) -> ! {
    panic!("out of memory")
}
