// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub use elf64::Elf;
use elf64::{ProgramSegmentType, Relocation};
use std::{
    librust::syscalls::{allocation::MemoryPermissions, vmspace::VmspaceSpawnEnv},
    vmspace::Vmspace,
};

const PAGE_SIZE: usize = 4096;

#[allow(clippy::result_unit_err)]
pub fn load_elf(elf: &Elf) -> Result<(Vmspace, VmspaceSpawnEnv), ()> {
    let relocations = elf
        .relocations()
        .map(|reloc| match reloc {
            Relocation::Rel(rel) => (rel.offset as usize, reloc),
            Relocation::Rela(rela) => (rela.offset as usize, reloc),
        })
        .collect::<std::collections::BTreeMap<usize, Relocation>>();

    // See if we have a RELRO section to fix up
    let relro = elf
        .program_headers()
        .find(|header| header.r#type == ProgramSegmentType::GnuRelro)
        .map(|header| header.vaddr as usize);
    let vmspace = Vmspace::new();
    let mut task_load_base = 0;
    let mut segment_offset = 0;
    let mut pc = 0;
    let elf_entry = elf.header.entry as usize;

    for header in elf.load_segments() {
        let align = header.align as usize;
        let mem_size = header.memory_size as usize;
        let vaddr = header.vaddr as usize;
        let file_size = header.file_size as usize;
        let is_relro = Some(vaddr) == relro;

        // RELRO will override any other permission flags here, so check to
        // see if the region we just processed is the RELRO segment
        let permissions = match (is_relro, header.flags) {
            (true, _) => MemoryPermissions::READ,
            (false, 0b101) => MemoryPermissions::READ | MemoryPermissions::EXECUTE,
            (false, 0b110) => MemoryPermissions::READ | MemoryPermissions::WRITE,
            (false, 0b100) => MemoryPermissions::READ,
            (false, flags) => unreachable!("flags: {:#b}", flags),
        };

        // Need to align-up the segment offset we were given here
        let mut segment_load_base = round_up_to_next(segment_offset, align);
        // Grab the bottom bits that we need to start writing data at
        let segment_load_offset = vaddr & (align - 1);
        // The total size in memory rounded up to the next alignment for the
        // segment
        let region_size = round_up_to_next(mem_size + segment_load_offset, align);

        let mut object = vmspace.create_object(segment_offset as *const _, region_size, permissions).unwrap();

        if task_load_base == 0 {
            segment_load_base = object.vmspace_address() as usize;
            task_load_base = object.vmspace_address() as usize;
        }

        assert!(align.is_power_of_two(), "ELF segment alignment isn't a power of two!");
        assert!(mem_size >= file_size, "ELF segment has less data in memory than in the file?");

        // Copy the segment data starting at the offset
        object.as_slice()[segment_load_offset..][..file_size].copy_from_slice(elf.program_segment_data(&header));

        // We use these values to key off of some information (e.g.
        // relocation calculations and calculating the PC)
        let raw_segment_start = header.vaddr as usize;
        let raw_segment_end = raw_segment_start + header.memory_size as usize;
        let raw_segment_range = raw_segment_start..raw_segment_end;

        // The real PC needs calculated from the offset, so we check to see
        // if this is the segment that contains the entry point
        if raw_segment_range.contains(&elf_entry) {
            let offset = elf_entry - raw_segment_start + segment_load_offset;
            pc = segment_load_base + offset;
        }

        // Find any relocations and fix them up before we write the memory
        // so we don't need to deal with the `UniquePhysicalRegion` which
        // doesn't play nice with arbitrary indexing since the physical
        // pages aren't guaranteed to be contiguous here so we can reuse
        // memory
        for (_, relocation) in relocations.range(raw_segment_start..raw_segment_end) {
            match relocation {
                Relocation::Rel(_) => todo!("rel relocations"),
                Relocation::Rela(rela) => {
                    let offset_into = rela.offset as usize - raw_segment_start + segment_load_offset;

                    match rela.r#type {
                        // RELATIVE
                        3 => {
                            // FIXME: Should prob check for negative addends?
                            let fixup = task_load_base + rela.addend as usize;
                            object.as_slice()[offset_into..][..8].copy_from_slice(&fixup.to_le_bytes());
                        }
                        n => todo!("relocation type: {}", n),
                    }
                }
            }
        }

        segment_offset = segment_load_base + region_size;
    }

    let tls = elf.program_headers().find(|header| header.r#type == elf64::ProgramSegmentType::Tls).map(|header| {
        // This is mostly the same as the above, just force 4 KiB alignment
        // because its not like we can have 8-byte aligned pages.
        //
        // TODO: `.tbss`?

        // This might actually not be necessary, since in the end the
        // thread-local loads are done with `tp + offset` but in case this
        // is important for any possible TLS relocations later, keeping it
        // the same as above
        let segment_load_offset = (header.vaddr as usize & (PAGE_SIZE - 1)) as usize;

        let mut tls_base = vmspace
            .create_object(
                core::ptr::null(),
                header.memory_size as usize + segment_load_offset,
                MemoryPermissions::READ | MemoryPermissions::WRITE,
            )
            .unwrap();

        let segment_file_size = header.file_size as usize;
        let data = tls_base.as_slice();
        data[segment_load_offset..][..segment_file_size].copy_from_slice(elf.program_segment_data(&header));
        data[segment_load_offset..][segment_file_size..].fill(0);

        tls_base.vmspace_address() as usize + segment_load_offset
    });

    let sp = vmspace
        .create_object(core::ptr::null(), 16 * PAGE_SIZE, MemoryPermissions::READ | MemoryPermissions::WRITE)
        .unwrap();
    let sp = sp.vmspace_address() as usize + 16 * PAGE_SIZE;

    Ok((vmspace, VmspaceSpawnEnv { pc, a0: 0, a1: 0, a2: 0, tp: tls.unwrap_or(0), sp }))
}

pub fn round_up_to_next(n: usize, size: usize) -> usize {
    assert!(size.is_power_of_two());

    if n % size == 0 {
        n
    } else {
        (n & !(size - 1)) + size
    }
}
