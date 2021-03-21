// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use fdt::Fdt;
use kernel_patching::kernel_section_p2v;

use crate::{
    csr::satp::{Satp, SatpMode},
    mem::{
        kernel_patching,
        paging::{PageSize, PhysicalAddress, Sv39PageTable, VirtualAddress, EXECUTE, READ, WRITE},
        phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
    },
    utils::{LinkerSymbol, StaticMut, Units},
};

extern "C" {
    static __bss_start: LinkerSymbol;
    static __bss_end: LinkerSymbol;
    static __data_start: LinkerSymbol;
    static __data_end: LinkerSymbol;
    static __text_start: LinkerSymbol;
    static __text_end: LinkerSymbol;
    static __tdata_start: LinkerSymbol;
    static __tdata_end: LinkerSymbol;
    static PHYS_OFFSET_VALUE: usize;
}

pub static TEMP_PAGE_TABLE_IDENTITY: StaticMut<Sv39PageTable> = StaticMut::new(Sv39PageTable::new());
pub static PAGE_TABLE_ROOT: StaticMut<Sv39PageTable> = StaticMut::new(Sv39PageTable::new());

/// # Safety
/// no
#[no_mangle]
pub unsafe extern "C" fn early_paging(hart_id: usize, fdt: *const u8, phys_load: usize) -> ! {
    let fdt_struct: Fdt<'static> = match fdt::Fdt::from_ptr(fdt) {
        Ok(fdt) => fdt,
        Err(e) => crate::platform::exit(crate::platform::ExitStatus::Error(&e)),
    };

    let fdt_size = fdt_struct.total_size() as u64;

    let page_offset_value = kernel_patching::page_offset();
    // These are physical addresses before paging is enabled
    let kernel_start = kernel_patching::kernel_start() as usize;
    let kernel_end = kernel_patching::kernel_end() as usize;

    let memory_region = fdt_struct
        .memory()
        .regions()
        .find(|region| {
            let start = region.starting_address as usize;
            let end = region.starting_address as usize + region.size.unwrap();

            start <= kernel_start && kernel_end <= end
        })
        .expect("wtf");

    let start = memory_region.starting_address as usize;
    let size = memory_region.size.unwrap() as usize;

    let kernel_end_phys = kernel_end as *mut u8;

    let mut pf_alloc = PHYSICAL_MEMORY_ALLOCATOR.lock();
    pf_alloc.init(kernel_end_phys, (start + size) as *mut u8);

    if fdt > kernel_end_phys {
        let n_pages = fdt_size as usize / 4096 + 1;
        for i in 0..n_pages {
            pf_alloc.set_used(crate::mem::phys::PhysicalPage::from_ptr(fdt.add(i * 4096) as *mut _));
        }
    }

    drop(pf_alloc);

    let bss_start = __bss_start.as_usize();
    let bss_end = __bss_end.as_usize();

    for addr in (bss_start..bss_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        (&mut *PAGE_TABLE_ROOT.get()).map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            PageSize::Kilopage,
            READ | WRITE,
        );
    }

    let data_start = __data_start.as_usize();
    let data_end = __data_end.as_usize();

    for addr in (data_start..data_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        (&mut *PAGE_TABLE_ROOT.get()).map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            PageSize::Kilopage,
            READ | WRITE,
        );
    }

    let text_start = __text_start.as_usize();
    let text_end = __text_end.as_usize();

    for addr in (text_start..text_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        (&mut *PAGE_TABLE_ROOT.get()).map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            PageSize::Kilopage,
            READ | EXECUTE,
        );
    }

    let ktls_start = __tdata_start.as_usize();
    let ktls_end = __tdata_end.as_usize();

    for addr in (ktls_start..ktls_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        (&mut *PAGE_TABLE_ROOT.get()).map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            PageSize::Kilopage,
            READ,
        );
    }

    for addr in 0..64 {
        (&mut *PAGE_TABLE_ROOT.get()).map(
            PhysicalAddress::new(addr * 1.gib()),
            VirtualAddress::new(PHYS_OFFSET_VALUE + addr * 1.gib()),
            PageSize::Gigapage,
            READ | WRITE,
        );
    }

    // This ***must*** go after all of the above initial paging code so that
    // addresses are identity mapped for page frame allocation
    crate::mem::PHYSICAL_OFFSET.store(PHYS_OFFSET_VALUE, core::sync::atomic::Ordering::Relaxed);

    let sp: usize;
    let gp: usize;
    asm!("lla {}, __tmp_stack_top", out(reg) sp);
    asm!("lla {}, __global_pointer$", out(reg) gp);

    let new_sp = (sp - phys_load) + page_offset_value;
    let new_gp = (gp - phys_load) + page_offset_value;

    let kmain_virt = kernel_section_p2v(PhysicalAddress::from_ptr(crate::kmain as *const u8));
    crate::csr::stvec::set(core::mem::transmute(kmain_virt.as_usize()));

    let satp = Satp { mode: SatpMode::Sv39, asid: 0, root_page_table: PhysicalAddress::from_ptr(&PAGE_TABLE_ROOT) };
    let fdt = crate::mem::phys2virt(PhysicalAddress::from_ptr(fdt)).as_ptr();

    #[rustfmt::skip]
    asm!(
        "
            # Set up stack pointer and global pointer
            mv sp, {new_sp}
            mv gp, {new_gp}

            # Load new `satp` value
            csrw satp, {satp}
            sfence.vma
            nop                 # we trap here and bounce to `kmain`!
        ",

        satp = in(reg) satp.to_usize(),
        new_sp = in(reg) new_sp,
        new_gp = in(reg) new_gp,

        // `kmain` arguments
        in("a0") hart_id,
        in("a1") fdt,
        options(noreturn, nostack),
    );
}
