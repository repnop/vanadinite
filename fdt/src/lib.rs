#![feature(arbitrary_self_types)]
#![no_std]

mod node;

use common::byteorder::{BigEndianU32, BigEndianU64};
use cstr_core::CStr;
pub use node::{MappedArea, MemoryNode, MemoryRegion, NodeProperty};

#[repr(C)]
pub struct Fdt {
    /// FDT header magic
    magic: BigEndianU32,
    /// Total size in bytes of the FDT structure
    totalsize: BigEndianU32,
    /// Offset in bytes from the start of the header to the structure block
    off_dt_struct: BigEndianU32,
    /// Offset in bytes from the start of the header to the strings block
    off_dt_strings: BigEndianU32,
    /// Offset in bytes from the start of the header to the memory reservation
    /// block
    off_mem_rsvmap: BigEndianU32,
    /// FDT version
    version: BigEndianU32,
    /// Last compatible FDT version
    last_comp_version: BigEndianU32,
    /// System boot CPU ID
    boot_cpuid_phys: BigEndianU32,
    /// Length in bytes of the strings block
    size_dt_strings: BigEndianU32,
    /// Length in bytes of the struct block
    size_dt_struct: BigEndianU32,
}

impl Fdt {
    /// # Safety
    /// This function checks the pointer alignment and performs a read to verify
    /// the magic value. If the pointer is invalid this can result in undefined
    /// behavior.
    pub unsafe fn new(ptr: *const u8) -> Option<*const Self> {
        assert_eq!(ptr as usize % 4, 0);

        let this: *const Self = ptr.cast();

        match this.validate_magic() {
            true => Some(this),
            false => None,
        }
    }

    /// # Safety
    /// This reads from a pointer to `Self`, and if invalid can result in UB
    pub unsafe fn validate_magic(self: *const Self) -> bool {
        self.make_ref().magic.get() == 0xd00dfeed
    }

    /// # Safety
    /// Requires a valid pointer to `Self`
    pub unsafe fn strings(self: *const Self) -> *const FdtStrings {
        self.offset_bytes(self.make_ref().off_dt_strings.get() as usize).cast()
    }

    /// # Safety
    /// Requres a valid pointer to `Self` and the header must specify a valid
    /// memory offset into the memory reservation block
    pub unsafe fn memory_reservations<'a>(self: *const Self) -> &'a [MemoryReservation] {
        let offset = self.make_ref().off_mem_rsvmap.get() as usize;
        let mut length = 0;

        let mut ptr = self.offset_bytes(offset).cast::<MemoryReservation>();
        while (*ptr).address.get() != 0 && (*ptr).size.get() != 0 {
            length += 1;
            ptr = ptr.add(1);
        }

        core::slice::from_raw_parts(self.offset_bytes(offset).cast(), length)
    }

    /// # Safety
    /// yes
    pub unsafe fn structure_block(self: *const Self) -> *const u32 {
        self.offset_bytes(self.make_ref().off_dt_struct.get() as usize).cast()
    }

    /// # Safety
    /// yes this unsafe is made of unsafe
    pub unsafe fn find_node<'a>(self: *const Self, name: &str) -> Option<impl Iterator<Item = node::NodeProperty<'a>>> {
        Some(node::node_properties(node::find_node(self.structure_block().cast(), name, self)?, self.strings()))
    }

    /// # Safety
    /// I'm the captain now
    pub unsafe fn memory<'a>(self: *const Self) -> MemoryNode<'a> {
        let properties = self.find_node("/memory").expect("requires memory node");

        let mut regions = &[][..];
        let mut initial_mapped_area = None;

        for property in properties {
            match property.name {
                "reg" => {
                    assert_eq!(property.value.as_ptr() as usize % 8, 0);
                    regions = core::slice::from_raw_parts(
                        property.value.as_ptr() as *const MemoryRegion,
                        property.value.len() / 16,
                    );
                }
                "initial-mapped-area" => {
                    assert_eq!(property.value.as_ptr() as usize % 8, 0);

                    let mut ptr = property.value.as_ptr();
                    let effective_address: BigEndianU64 = *ptr.cast();

                    node::advance_ptr(&mut ptr, 8);
                    let physical_address: BigEndianU64 = *ptr.cast();

                    node::advance_ptr(&mut ptr, 8);
                    let size: BigEndianU32 = *ptr.cast();

                    initial_mapped_area = Some(MappedArea {
                        effective_address: effective_address.get(),
                        physical_address: physical_address.get(),
                        size: size.get(),
                    });
                }
                "device_type" => {}
                _ => unreachable!("bad memory node format"),
            }
        }

        MemoryNode { regions, initial_mapped_area }
    }

    unsafe fn make_ref<'a>(self: *const Self) -> &'a Self {
        &*self
    }

    unsafe fn offset_bytes(self: *const Self, n: usize) -> *const u8 {
        self.cast::<u8>().add(n)
    }
}

pub struct FdtStrings;

impl FdtStrings {
    /// # Safety
    /// Requires a valid pointer to `Self` and the offset in bytes must point to
    /// a valid C string
    pub unsafe fn cstr_at_offset<'a>(self: *const Self, offset: usize) -> &'a CStr {
        CStr::from_ptr(self.cast::<cstr_core::c_char>().add(offset))
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MemoryReservation {
    pub address: BigEndianU64,
    pub size: BigEndianU64,
}
