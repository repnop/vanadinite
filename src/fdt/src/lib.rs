#![no_std]

mod node;

use common::byteorder::{BigEndianU32, BigEndianU64};
use cstr_core::CStr;
pub use node::{Compatible, MappedArea, MemoryNode, MemoryRegion, NodeProperty};

#[derive(Debug)]
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
    pub unsafe fn new<'a>(ptr: *const u8) -> Option<&'a Self> {
        if ptr.is_null() || ptr.align_offset(4) != 0 {
            return None;
        }

        let this: &Self = &*ptr.cast();

        match this.validate_magic() {
            true => Some(this),
            false => None,
        }
    }

    /// # Safety
    /// This reads from a pointer to `Self`, and if invalid can result in UB
    fn validate_magic(&self) -> bool {
        self.magic.get() == 0xd00dfeed
    }

    pub fn strings(&self) -> impl Iterator<Item = &str> {
        let mut ptr = self.strings_ptr();

        core::iter::from_fn(move || {
            if ptr >= self.strings_limit() {
                return None;
            }

            let cstr = unsafe { CStr::from_ptr(ptr) };
            ptr = unsafe { ptr.add(cstr.to_bytes().len() + 1) };
            Some(cstr.to_str().ok()?)
        })
    }

    pub fn memory_reservations<'a>(&self) -> &'a [MemoryReservation] {
        let offset = self.off_mem_rsvmap.get() as usize;
        let mut length = 0;

        let mut ptr = self.offset_bytes(offset).cast::<MemoryReservation>();
        unsafe {
            while (*ptr).address.get() != 0 && (*ptr).size.get() != 0 {
                length += 1;
                ptr = ptr.add(1);
            }
        }

        unsafe { core::slice::from_raw_parts(self.offset_bytes(offset).cast(), length) }
    }

    /// Returns the first node that matches the node path, if you want all that
    /// match the path, use `find_all_nodes`
    pub fn find_node(&self, path: &str) -> Option<node::FdtNode<'_>> {
        unsafe { node::find_node(&mut self.structs_ptr().cast(), path, self, None) }
    }

    pub fn find_all_nodes(&self, path: &str) -> impl Iterator<Item = node::FdtNode<'_>> {
        // let parent_path = path.rsplitn('/');
        // let parent = unsafe { node::find_node(&mut self.structs_ptr().cast(), name, self) };
        None.into_iter()
    }

    pub fn chosen(&self) -> Option<Chosen<'_>> {
        unsafe { node::find_node(&mut self.structs_ptr().cast(), "/chosen", self, None) }.map(|node| Chosen { node })
    }

    pub fn memory(&self) -> MemoryNode<'_> {
        MemoryNode { node: self.find_node("/memory").expect("requires memory node") }
    }

    pub fn total_size(&self) -> usize {
        self.totalsize.get() as usize
    }

    fn limit(&self) -> *const u8 {
        unsafe { (self as *const Self).cast::<u8>().add(self.total_size()) }
    }

    fn cstr_at_offset<'a>(&self, offset: usize) -> &'a CStr {
        let ptr = unsafe { self.strings_ptr().add(offset) };
        assert!(ptr < self.limit(), "cstr past limit");
        unsafe { CStr::from_ptr(ptr) }
    }

    fn str_at_offset<'a>(&self, offset: usize) -> &'a str {
        self.cstr_at_offset(offset).to_str().expect("not utf-8 cstr")
    }

    fn strings_ptr(&self) -> *const u8 {
        self.offset_bytes(self.off_dt_strings.get() as usize)
    }

    fn strings_limit(&self) -> *const u8 {
        unsafe { self.offset_bytes(self.off_dt_strings.get() as usize).add(self.size_dt_strings.get() as usize) }
    }

    fn structs_ptr(&self) -> *const u8 {
        self.offset_bytes(self.off_dt_struct.get() as usize)
    }

    fn structs_limit(&self) -> *const u8 {
        unsafe { self.offset_bytes(self.off_dt_struct.get() as usize).add(self.size_dt_struct.get() as usize) }
    }

    fn offset_bytes(&self, n: usize) -> *const u8 {
        let ptr = unsafe { (self as *const Self).cast::<u8>().add(n) };
        assert!(ptr < self.limit(), "offset past limit");

        ptr
    }
}

pub struct Chosen<'a> {
    node: node::FdtNode<'a>,
}

impl<'a> Chosen<'a> {
    pub fn bootargs(self) -> Option<&'a str> {
        self.node
            .properties()
            .find(|n| n.name == "bootargs")
            .and_then(|n| core::str::from_utf8(&n.value[..n.value.len() - 1]).ok())
    }

    pub fn stdout(self) -> Option<node::FdtNode<'a>> {
        self.node
            .properties()
            .find(|n| n.name == "stdout-path")
            .and_then(|n| core::str::from_utf8(&n.value[..n.value.len() - 1]).ok())
            .and_then(|name| self.node.header.find_node(name))
    }

    pub fn stdin(self) -> Option<node::FdtNode<'a>> {
        self.node
            .properties()
            .find(|n| n.name == "stdin-path")
            .and_then(|n| core::str::from_utf8(&n.value[..n.value.len() - 1]).ok())
            .and_then(|name| self.node.header.find_node(name))
            .or_else(|| self.stdout())
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MemoryReservation {
    address: BigEndianU64,
    size: BigEndianU64,
}

impl MemoryReservation {
    pub fn address(&self) -> *const u8 {
        self.address.get() as usize as *const u8
    }

    pub fn size(&self) -> usize {
        self.size.get() as usize
    }
}

trait PtrHelpers {
    type Output;
    unsafe fn offset_bytes(&mut self, bytes: usize) -> Self::Output;
}

impl<T> PtrHelpers for *const T {
    type Output = *const T;
    unsafe fn offset_bytes(&mut self, bytes: usize) -> Self::Output {
        *self = (*self).cast::<u8>().add(bytes).cast();
        *self
    }
}
