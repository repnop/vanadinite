// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::{
        kernel_patching,
        paging::{Execute, PageSize, PhysicalAddress, Read, Sv39PageTable, VirtualAddress, Write},
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
    static __kernel_thread_local_start: LinkerSymbol;
    static __kernel_thread_local_end: LinkerSymbol;
}

pub static TEMP_PAGE_TABLE_IDENTITY: StaticMut<Sv39PageTable> = StaticMut::new(Sv39PageTable::new());
pub static PAGE_TABLE_ROOT: StaticMut<Sv39PageTable> = StaticMut::new(Sv39PageTable::new());

/// # Safety
/// no
#[no_mangle]
pub unsafe extern "C" fn early_paging(hart_id: usize, fdt: *const u8, phys_load: usize) -> ! {
    let kmain: usize;
    asm!("
        lla {tmp}, kmain_addr_virt
        ld {tmp}, ({tmp})
        mv {}, {tmp}
    ", out(reg) kmain, tmp = out(reg) _);

    let fdt_struct = match fdt::Fdt::new(fdt) {
        Some(fdt) => fdt,
        None => crate::arch::exit(crate::arch::ExitStatus::Error(&"magic's fucked, my dude")),
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

    // let start = 0x80000000;
    // let size = 0x8000000;

    let ident_mem_phys = (TEMP_PAGE_TABLE_IDENTITY.get(), PhysicalAddress::from_ptr(&TEMP_PAGE_TABLE_IDENTITY));

    let fdt_usize = fdt as usize;
    let fdt_phys = PhysicalAddress::new(fdt_usize);
    let fdt_virt = VirtualAddress::new(fdt_usize);

    //let alloc_start = if fdt_usize >= kernel_end {
    //    // round up to next page size after the FDT
    //    ((phys2virt(fdt_phys).as_usize() + fdt_size as usize) & !0x1FFFFFusize) + 0x200000
    //} else {
    //    kernel_end
    //};

    let kernel_end_phys = kernel_end as *mut u8;

    // Set the allocator to start allocating memory after the end of the kernel or fdt
    //let kernel_end_phys = mem::virt2phys(VirtualAddress::new(alloc_start)).as_mut_ptr();

    let mut pf_alloc = PHYSICAL_MEMORY_ALLOCATOR.lock();
    pf_alloc.init(kernel_end_phys, (start + size) as *mut u8);

    if fdt > kernel_end_phys {
        let n_pages = fdt_size as usize / 4096 + 1;
        for i in 0..n_pages {
            pf_alloc.set_used(crate::mem::phys::PhysicalPage::from_ptr(fdt.add(i * 4096) as *mut _));
        }
    }

    let mut page_alloc = || {
        let phys_addr = pf_alloc.alloc().unwrap().as_phys_address();
        (phys_addr.as_mut_ptr() as *mut Sv39PageTable, phys_addr)
    };

    for address in (kernel_start..kernel_end).step_by(2.mib()) {
        let ident = VirtualAddress::new(address);
        let phys = PhysicalAddress::new(address);
        let permissions = Read | Write | Execute;

        (&mut *PAGE_TABLE_ROOT.get()).map(
            phys,
            ident,
            PageSize::Megapage,
            permissions,
            || ident_mem_phys,
            |p| VirtualAddress::new(p.as_usize()),
        );
    }

    let bss_start = __bss_start.as_usize();
    let bss_end = __bss_end.as_usize();

    for addr in (bss_start..bss_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        (&mut *PAGE_TABLE_ROOT.get()).map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            PageSize::Kilopage,
            Read | Write,
            &mut page_alloc,
            |p| VirtualAddress::new(p.as_usize()),
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
            Read | Write,
            &mut page_alloc,
            |p| VirtualAddress::new(p.as_usize()),
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
            Read | Execute,
            &mut page_alloc,
            |p| VirtualAddress::new(p.as_usize()),
        );
    }

    let ktls_start = __kernel_thread_local_start.as_usize();
    let ktls_end = __kernel_thread_local_end.as_usize();

    for addr in (ktls_start..ktls_end).step_by(4096) {
        let addr = PhysicalAddress::new(addr);
        (&mut *PAGE_TABLE_ROOT.get()).map(
            addr,
            crate::kernel_patching::kernel_section_p2v(addr),
            PageSize::Kilopage,
            Read | Write,
            &mut page_alloc,
            |p| VirtualAddress::new(p.as_usize()),
        );
    }

    for addr in 0..64 {
        (&mut *PAGE_TABLE_ROOT.get()).map(
            PhysicalAddress::new(addr * 1.gib()),
            VirtualAddress::new(0xFFFFFFC000000000 + addr * 1.gib()),
            PageSize::Gigapage,
            Read | Write,
            &mut page_alloc,
            |p| VirtualAddress::new(p.as_usize()),
        );
    }

    #[cfg(feature = "virt")]
    (&mut *PAGE_TABLE_ROOT.get()).map(
        PhysicalAddress::new(0x10_0000),
        VirtualAddress::new(0x10_0000),
        PageSize::Kilopage,
        Read | Write,
        &mut page_alloc,
        |p| VirtualAddress::new(p.as_usize()),
    );

    if !(&*PAGE_TABLE_ROOT.get()).is_mapped(fdt_virt, |p| VirtualAddress::new(p.as_usize())) {
        let rounded_up = ((fdt_phys.as_usize() + fdt_size as usize) & !0xFFF) + 0x1000;

        for addr in (fdt_phys.as_usize()..rounded_up).step_by(4096) {
            let fdt_phys = PhysicalAddress::new(addr);
            let fdt_virt = VirtualAddress::new(addr);
            (&mut *PAGE_TABLE_ROOT.get()).map(fdt_phys, fdt_virt, PageSize::Kilopage, Read, &mut page_alloc, |p| {
                VirtualAddress::new(p.as_usize())
            });
        }
    }

    drop(pf_alloc);

    let sp: usize;
    let gp: usize;
    asm!("lla {}, __tmp_stack_top", out(reg) sp);
    asm!("lla {}, __global_pointer$", out(reg) gp);

    let new_sp = (sp - phys_load) + page_offset_value;
    let new_gp = (gp - phys_load) + page_offset_value;

    crate::mem::satp(crate::mem::SatpMode::Sv39, 0, PhysicalAddress::from_ptr(&PAGE_TABLE_ROOT));
    crate::mem::sfence(None, None);

    vmem_trampoline(hart_id, fdt, new_sp, new_gp, kmain)
}

extern "C" {
    fn vmem_trampoline(_: usize, _: *const u8, sp: usize, gp: usize, dest: usize) -> !;
}

#[rustfmt::skip]
global_asm!("
    .section .text
    .globl vmem_trampoline
    vmem_trampoline:
        mv sp, a2
        mv gp, a3
        jr a4
");
