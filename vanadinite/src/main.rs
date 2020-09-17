#![allow(clippy::match_bool)]
#![feature(asm, naked_functions, global_asm, alloc_error_handler, raw_ref_op)]
#![no_std]
#![no_main]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("vanadinite assumes a 64-bit pointer size, cannot compile on non-64 bit systems");

mod arch;
mod asm;
mod drivers;
mod io;
mod mem;
mod sync;
mod trap;
mod utils;

use core::cell::UnsafeCell;
use mem::{
    kernel_patching, perms,
    sv39::{self, PageSize, PhysicalAddress, Sv39PageTable, VirtualAddress},
};

/// # Safety
/// I'm the kernel, rustc
#[naked]
#[no_mangle]
#[link_section = ".init.boot"]
pub unsafe extern "C" fn _boot() -> ! {
    #[rustfmt::skip]
    asm!("
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        lla t0, __bss_start
        lla t1, __bss_end

        clear_bss:
            beq t0, t1, done_clear_bss
            sd zero, (t0)
            addi t0, t0, 8
            j clear_bss

        done_clear_bss:

        lla sp, __tmp_stack_top

        lla a2, PAGE_OFFSET
        lla t0, KERNEL_PHYS_LOAD_LOCATION
        sd a2, (t0)

        j early_paging

        .section .data
        .globl boot_entry_addr_virt
        boot_entry_addr_virt: .dword boot_entry
        .globl PAGE_OFFSET_VALUE
        PAGE_OFFSET_VALUE: .dword PAGE_OFFSET
    ");

    loop {}
}

#[repr(transparent)]
struct StaticPageTable(UnsafeCell<Sv39PageTable>);

unsafe impl Send for StaticPageTable {}
unsafe impl Sync for StaticPageTable {}

static TEMP_PAGE_TABLE_ROOT: StaticPageTable = StaticPageTable(UnsafeCell::new(Sv39PageTable::new()));
static TEMP_PAGE_TABLE_HIGH_MEM: StaticPageTable = StaticPageTable(UnsafeCell::new(Sv39PageTable::new()));
static TEMP_PAGE_TABLE_IDENTITY: StaticPageTable = StaticPageTable(UnsafeCell::new(Sv39PageTable::new()));

/// # Safety
/// no
#[no_mangle]
pub unsafe extern "C" fn early_paging(hart_id: usize, fdt: *const u8, phys_load: usize) -> ! {
    let boot_entry_addr: usize;
    asm!("
        lla {tmp}, boot_entry_addr_virt
        ld {tmp}, ({tmp})
        mv {}, {tmp}
    ", out(reg) boot_entry_addr, tmp = out(reg) _);

    let fdt_struct = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => arch::exit(arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let page_offset_value = kernel_patching::page_offset();
    // These are physical addresses before paging is enabled
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;

    let memory_region = fdt_struct
        .memory()
        .regions
        .iter()
        .copied()
        .find(|region| {
            let start = region.starting_address() as usize;
            let end = (region.starting_address() + region.size()) as usize;

            start <= kernel_start && kernel_start <= end
        })
        .expect("wtf");

    let high_mem_phys = (TEMP_PAGE_TABLE_HIGH_MEM.0.get(), PhysicalAddress::from_ptr(&TEMP_PAGE_TABLE_HIGH_MEM));
    let ident_mem_phys = (TEMP_PAGE_TABLE_IDENTITY.0.get(), PhysicalAddress::from_ptr(&TEMP_PAGE_TABLE_IDENTITY));

    for address in (kernel_start..(kernel_end + TWO_MEBS)).step_by(TWO_MEBS) {
        let virt = VirtualAddress::new(page_offset_value + (address - kernel_start));
        let ident = VirtualAddress::new(address);
        let phys = PhysicalAddress::new(address);
        let permissions = perms::Read | perms::Write | perms::Execute;

        (&mut *TEMP_PAGE_TABLE_ROOT.0.get()).map(phys, virt, PageSize::Megapage, permissions, || high_mem_phys);
        (&mut *TEMP_PAGE_TABLE_ROOT.0.get()).map(phys, ident, PageSize::Megapage, permissions, || ident_mem_phys);
    }

    mem::satp(mem::SatpMode::Sv39, 0, PhysicalAddress::from_ptr(&TEMP_PAGE_TABLE_ROOT));
    mem::sfence();
    let boot_entry_addr = core::mem::transmute::<_, fn(usize, *const u8, fdt::MemoryRegion) -> !>(boot_entry_addr);

    let sp: usize;
    let gp: usize;
    asm!("lla {}, __tmp_stack_top", out(reg) sp);
    asm!("lla {}, __global_pointer$", out(reg) gp);

    let new_sp = (sp - phys_load) + page_offset_value;
    let new_gp = (gp - phys_load) + page_offset_value;
    asm!("mv sp, {}", in(reg) new_sp);
    asm!("mv gp, {}", in(reg) new_gp);

    boot_entry_addr(hart_id, fdt, memory_region)
}

const TWO_MEBS: usize = 2 * 1024 * 1024;

/// # Safety
/// Uh, probably none
#[no_mangle]
pub unsafe extern "C" fn boot_entry(hart_id: usize, fdt: *const u8, region: fdt::MemoryRegion) -> ! {
    if hart_id != 0 {
        panic!("not hart 0");
    }

    // Remove identity mapping after paging initialization
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;
    for address in (kernel_start..(kernel_end + TWO_MEBS)).step_by(TWO_MEBS) {
        // `kernel_start()` and `kernel_end()` now refer to virtual addresses so
        // we need to patch them back to physical "virtual" addresses to be
        // unmapped
        let patched = VirtualAddress::new(kernel_patching::virt2phys(VirtualAddress::new(address)).as_usize());
        (&mut *TEMP_PAGE_TABLE_ROOT.0.get()).unmap(patched, kernel_patching::phys2virt);
    }

    io::init_logging();

    let fdt = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => arch::exit(arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let uart_fdt = fdt.find_node("/uart");

    for property in uart_fdt.unwrap() {
        if let Some(reg) = property.reg() {
            io::set_console(reg.starting_address() as usize as *mut drivers::uart16550::Uart16550);
        }
    }

    log::info!("# of memory reservations: {}", fdt.memory_reservations().len());
    log::info!("{:#x?}", fdt.memory());

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
