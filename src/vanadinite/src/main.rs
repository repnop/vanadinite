#![allow(clippy::match_bool)]
#![feature(asm, naked_functions, global_asm, alloc_error_handler, raw_ref_op)]
#![no_std]
#![no_main]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("vanadinite assumes a 64-bit pointer size, cannot compile on non-64 bit systems");

extern crate alloc;

struct Heck;

unsafe impl alloc::alloc::GlobalAlloc for Heck {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        todo!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        todo!()
    }
}

#[global_allocator]
static HECK: Heck = Heck;

mod arch;
mod asm;
mod drivers;
mod io;
mod mem;
mod sync;
// mod trap;
mod utils;

use core::cell::UnsafeCell;
use mem::{
    kernel_patching,
    paging::{Execute, PageSize, PhysicalAddress, Read, Sv39PageTable, ToPermissions, VirtualAddress, Write},
    phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
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

    let fdt_size = fdt_struct.total_size() as u64;

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

    for address in (kernel_start..(kernel_end + TWO_MEBS + TWO_MEBS)).step_by(TWO_MEBS) {
        let virt = VirtualAddress::new(page_offset_value + (address - kernel_start));
        let ident = VirtualAddress::new(address);
        let phys = PhysicalAddress::new(address);
        let permissions = Read | Write | Execute;

        (&mut *TEMP_PAGE_TABLE_ROOT.0.get()).map(
            phys,
            virt,
            PageSize::Megapage,
            permissions,
            || high_mem_phys,
            |p| VirtualAddress::new(p.as_usize()),
        );
        (&mut *TEMP_PAGE_TABLE_ROOT.0.get()).map(
            phys,
            ident,
            PageSize::Megapage,
            permissions,
            || ident_mem_phys,
            |p| VirtualAddress::new(p.as_usize()),
        );
    }

    let start = memory_region.starting_address();
    let size = memory_region.size();

    let sp: usize;
    let gp: usize;
    asm!("lla {}, __tmp_stack_top", out(reg) sp);
    asm!("lla {}, __global_pointer$", out(reg) gp);

    let new_sp = (sp - phys_load) + page_offset_value;
    let new_gp = (gp - phys_load) + page_offset_value;

    mem::satp(mem::SatpMode::Sv39, 0, PhysicalAddress::from_ptr(&TEMP_PAGE_TABLE_ROOT));
    mem::sfence();

    vmem_trampoline(hart_id, fdt, start, size, new_sp, new_gp, boot_entry_addr, fdt_size)
}

extern "C" {
    fn vmem_trampoline(_: usize, _: *const u8, _: u64, _: u64, sp: usize, gp: usize, dest: usize, _: u64) -> !;
}

#[rustfmt::skip]
global_asm!("
    .section .text
    .globl vmem_trampoline
    vmem_trampoline:
        mv sp, a4
        mv gp, a5
        mv a4, a7
        jr a6
");

const TWO_MEBS: usize = 2 * 1024 * 1024;

/// # Safety
/// Uh, probably none
#[no_mangle]
pub unsafe extern "C" fn boot_entry(
    hart_id: usize,
    fdt: *const u8,
    region_start: u64,
    region_size: u64,
    fdt_size: u64,
) -> ! {
    let region_start = region_start as usize;
    let region_size = region_size as usize;

    if hart_id != 0 {
        panic!("not hart 0");
    }

    let root_page_table = &mut *TEMP_PAGE_TABLE_ROOT.0.get();

    // Remove identity mapping after paging initialization
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;
    for address in (kernel_start..(kernel_end + TWO_MEBS + TWO_MEBS)).step_by(TWO_MEBS) {
        // `kernel_start()` and `kernel_end()` now refer to virtual addresses so
        // we need to patch them back to physical "virtual" addresses to be
        // unmapped
        let patched = VirtualAddress::new(kernel_patching::virt2phys(VirtualAddress::new(address)).as_usize());
        root_page_table.unmap(patched, kernel_patching::phys2virt);
    }

    let fdt_usize = fdt as usize;
    let fdt_phys = PhysicalAddress::new(fdt_usize);
    let fdt_virt = VirtualAddress::new(fdt_usize);

    let alloc_start = if fdt as usize >= kernel_patching::virt2phys(VirtualAddress::new(kernel_end)).as_usize() {
        // round up to next page size after the FDT
        ((kernel_patching::phys2virt(fdt_phys).as_usize() + fdt_size as usize) & !0x1FFFFFusize) + 0x200000
    } else {
        kernel_end
    };

    if !root_page_table.is_mapped(VirtualAddress::new(alloc_start), kernel_patching::phys2virt) {
        let alloc_start = VirtualAddress::new(alloc_start);
        let high_mem_phys = kernel_patching::virt2phys(VirtualAddress::from_ptr(&TEMP_PAGE_TABLE_HIGH_MEM));
        root_page_table.map(
            kernel_patching::virt2phys(alloc_start),
            alloc_start,
            PageSize::Megapage,
            Read | Write | Execute,
            || (TEMP_PAGE_TABLE_HIGH_MEM.0.get(), high_mem_phys),
            kernel_patching::phys2virt,
        );
    }

    // Set the allocator to start allocating memory after the end of the kernel or fdt
    let kernel_end_phys = kernel_patching::virt2phys(VirtualAddress::new(alloc_start)).as_mut_ptr();

    let mut pf_alloc = PHYSICAL_MEMORY_ALLOCATOR.lock();
    pf_alloc.init(kernel_end_phys, (region_start + region_size) as *mut u8);

    let mut page_alloc = || {
        let phys_addr = pf_alloc.alloc().unwrap().as_phys_address();
        let virt_addr = kernel_patching::phys2virt(phys_addr);

        (virt_addr.as_mut_ptr() as *mut Sv39PageTable, phys_addr)
    };

    #[cfg(feature = "virt")]
    root_page_table.map(
        PhysicalAddress::new(0x10_0000),
        VirtualAddress::new(0x10_0000),
        PageSize::Kilopage,
        Read | Write | Execute,
        &mut page_alloc,
        kernel_patching::phys2virt,
    );

    if !root_page_table.is_mapped(fdt_virt, kernel_patching::phys2virt) {
        root_page_table.map(fdt_phys, fdt_virt, PageSize::Kilopage, Read, &mut page_alloc, kernel_patching::phys2virt);
    }

    io::init_logging();

    let fdt = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => arch::exit(arch::ExitStatus::Error(&"magic's fucked, my dude")),
    };

    let uart_fdt = fdt.find_node("/uart");

    for property in uart_fdt.unwrap() {
        if let Some(reg) = property.reg() {
            let uart_addr = reg.starting_address() as usize;
            let uart_phys = PhysicalAddress::new(uart_addr);
            let uart_virt = VirtualAddress::new(uart_addr);
            let perms = Read | Write;
            root_page_table.map(
                uart_phys,
                uart_virt,
                PageSize::Kilopage,
                perms,
                &mut page_alloc,
                kernel_patching::phys2virt,
            );

            io::set_console(uart_addr as *mut drivers::uart16550::Uart16550);
        }
    }

    let page = pf_alloc.alloc().unwrap();
    log::info!("{:?}", page);
    pf_alloc.dealloc(page);

    drop(pf_alloc);

    log::info!("# of memory reservations: {}\r", fdt.memory_reservations().len());
    log::info!("{:#x?}", fdt.memory());

    log::info!("SBI spec version: {:?}", sbi::base::spec_version());
    log::info!("SBI implementor: {:?}", sbi::base::impl_id());
    log::info!("marchid: {:#x}", sbi::base::marchid());

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
