// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::AtomicUsize;

use fdt::Fdt;
use kernel_patching::kernel_section_p2v;

use crate::{
    csr::satp::Satp,
    mem::{
        kernel_patching,
        paging::{
            flags::{ACCESSED, DIRTY, EXECUTE, READ, VALID, WRITE},
            PageSize, PageTable, PhysicalAddress, VirtualAddress, SATP_MODE,
        },
        phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
    },
    utils::{LinkerSymbol, Units},
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

pub static BOOTSTRAP_SATP: AtomicUsize = AtomicUsize::new(0);

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

    let mut root_page_table = PageTable::new_raw();

    let bss_start = __bss_start.as_usize();
    let bss_end = __bss_end.as_usize();

    for addr in (bss_start..bss_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        root_page_table.static_map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            DIRTY | ACCESSED | READ | WRITE | VALID,
            PageSize::Kilopage,
        );
    }

    let data_start = __data_start.as_usize();
    let data_end = __data_end.as_usize();

    for addr in (data_start..data_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        root_page_table.static_map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            DIRTY | ACCESSED | READ | WRITE | VALID,
            PageSize::Kilopage,
        );
    }

    let text_start = __text_start.as_usize();
    let text_end = __text_end.as_usize();

    for addr in (text_start..text_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        root_page_table.static_map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            ACCESSED | EXECUTE | VALID,
            PageSize::Kilopage,
        );
    }

    let ktls_start = __tdata_start.as_usize();
    let ktls_end = __tdata_end.as_usize();

    for addr in (ktls_start..ktls_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        root_page_table.static_map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            ACCESSED | READ | VALID,
            PageSize::Kilopage,
        );
    }

    for addr in 0..64 {
        root_page_table.static_map(
            PhysicalAddress::new(addr * 1.gib()),
            VirtualAddress::new(PHYS_OFFSET_VALUE + addr * 1.gib()),
            DIRTY | ACCESSED | READ | WRITE | VALID,
            PageSize::Gigapage,
        );
    }

    // Need to leak the root page table here so it doesn't drop
    let root_pt_phys = root_page_table.physical_address();
    core::mem::forget(root_page_table);

    let satp = Satp { mode: SATP_MODE, asid: 0, root_page_table: root_pt_phys };

    BOOTSTRAP_SATP.store(satp.as_usize(), core::sync::atomic::Ordering::SeqCst);

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

    let fdt = crate::mem::phys2virt(PhysicalAddress::from_ptr(fdt)).as_ptr();

    #[rustfmt::skip]
    asm!(
        "
            # Set up stack pointer and global pointer
            mv sp, {new_sp}
            mv gp, {new_gp}

            csrs sstatus, {mxr}

            # Load new `satp` value
            csrw satp, {satp}
            sfence.vma
            nop                 # we trap here and bounce to `kmain`!
        ",
        mxr = in(reg) 1 << 19,
        satp = in(reg) satp.as_usize(),
        new_sp = in(reg) new_sp,
        new_gp = in(reg) new_gp,

        // `kmain` arguments
        in("a0") hart_id,
        in("a1") fdt,
        options(noreturn, nostack),
    );
}
