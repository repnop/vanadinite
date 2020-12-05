// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

mod node;

use common::byteorder::{BigEndianU32, BigEndianU64, FromBytes, IntegerStream};
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

    pub fn root(&self) -> Root<'_> {
        Root { node: self.find_node("/").expect("/ is a required node") }
    }

    pub fn aliases(&self) -> Aliases<'_> {
        Aliases { node: self.find_node("/aliases").expect("/aliases is a required node"), header: self }
    }

    pub fn cpus(&self) -> impl Iterator<Item = Cpu<'_>> + '_ {
        let parent = self.find_node("/cpus").expect("/cpus is a required node");

        parent
            .children()
            .filter(|c| c.name.split('@').next().unwrap() == "cpu")
            .map(move |cpu| Cpu { parent, node: cpu })
    }

    /// Returns the first node that matches the node path, if you want all that
    /// match the path, use `find_all_nodes`. This will automatically attempt to
    /// resolve aliases if `path` is not found.
    pub fn find_node(&self, path: &str) -> Option<node::FdtNode<'_>> {
        let node = unsafe { node::find_node(&mut self.structs_ptr().cast(), path, self, None) };
        node.or_else(|| self.aliases().resolve_node(path))
    }

    pub fn find_all_nodes<'a>(&'a self, path: &'a str) -> impl Iterator<Item = node::FdtNode<'a>> {
        let mut done = false;
        let only_root = path == "/";
        let valid_path = path.chars().fold(0, |acc, c| acc + if c == '/' { 1 } else { 0 }) >= 1;

        let mut path_split = path.rsplitn(2, '/');
        let child_name = path_split.next().unwrap();
        let parent_path = match path_split.next().unwrap() {
            "" => "/",
            s => s,
        };
        let parent = unsafe { node::find_node(&mut self.structs_ptr().cast(), parent_path, self, None) };
        let (parent, bad_parent) = match parent {
            Some(parent) => (parent, false),
            None => (self.find_node("/").unwrap(), true),
        };

        let mut child_iter = parent.children();

        core::iter::from_fn(move || {
            if done || !valid_path || bad_parent {
                return None;
            }

            if only_root {
                done = true;
                return self.find_node("/");
            }

            let mut ret = None;

            #[allow(clippy::while_let_on_iterator)]
            while let Some(child) = child_iter.next() {
                if child.name.split('@').next()? == child_name {
                    ret = Some(child);
                    break;
                }
            }

            ret
        })
    }

    pub fn all_nodes(&self) -> impl Iterator<Item = node::FdtNode<'_>> {
        unsafe { node::all_nodes(self) }
    }

    pub fn find_phandle(&self, phandle: u32) -> Option<node::FdtNode<'_>> {
        self.all_nodes().find(|n| {
            n.properties()
                .find(|p| p.name == "phandle")
                .and_then(|p| Some(BigEndianU32::from_bytes(p.value)?.get() == phandle))
                .unwrap_or(false)
        })
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
pub struct Root<'a> {
    node: node::FdtNode<'a>,
}

impl<'a> Root<'a> {
    pub fn cell_sizes(self) -> node::CellSizes {
        self.node.cell_sizes()
    }

    pub fn model(self) -> &'a str {
        self.node.properties().find(|p| p.name == "model").and_then(|p| core::str::from_utf8(p.value).ok()).unwrap()
    }

    pub fn compatible(self) -> Compatible<'a> {
        self.node.compatible().unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Aliases<'a> {
    header: &'a Fdt,
    node: node::FdtNode<'a>,
}

impl<'a> Aliases<'a> {
    pub fn resolve(self, alias: &str) -> Option<&'a str> {
        self.node.properties().find(|p| p.name == alias).and_then(|p| core::str::from_utf8(p.value).ok())
    }

    pub fn resolve_node(self, alias: &str) -> Option<node::FdtNode<'a>> {
        self.resolve(alias).and_then(|name| self.header.find_node(name))
    }

    pub fn all(self) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.node.properties().filter_map(|p| Some((p.name, core::str::from_utf8(p.value).ok()?)))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cpu<'a> {
    parent: node::FdtNode<'a>,
    node: node::FdtNode<'a>,
}

impl<'a> Cpu<'a> {
    pub fn ids(self) -> CpuIds<'a> {
        let address_cells = self.node.cell_sizes().address_cells;

        CpuIds { reg: self.node.properties().find(|p| p.name == "reg").unwrap(), address_cells }
    }

    pub fn clock_frequency(self) -> usize {
        self.node
            .properties()
            .find(|p| p.name == "clock-frequency")
            .or_else(|| self.parent.properties().find(|p| p.name == "clock-frequency"))
            .map(|p| match p.value.len() {
                4 => BigEndianU32::from_bytes(p.value).unwrap().get() as usize,
                8 => BigEndianU64::from_bytes(p.value).unwrap().get() as usize,
                _ => unreachable!(),
            })
            .unwrap()
    }

    pub fn timebase_frequency(self) -> usize {
        self.node
            .properties()
            .find(|p| p.name == "timebase-frequency")
            .or_else(|| self.parent.properties().find(|p| p.name == "timebase-frequency"))
            .map(|p| match p.value.len() {
                4 => BigEndianU32::from_bytes(p.value).unwrap().get() as usize,
                8 => BigEndianU64::from_bytes(p.value).unwrap().get() as usize,
                _ => unreachable!(),
            })
            .unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpuIds<'a> {
    reg: node::NodeProperty<'a>,
    address_cells: usize,
}

impl<'a> CpuIds<'a> {
    pub fn first(self) -> usize {
        match self.address_cells {
            1 => BigEndianU32::from_bytes(self.reg.value).unwrap().get() as usize,
            2 => BigEndianU64::from_bytes(self.reg.value).unwrap().get() as usize,
            n => panic!("address-cells of size {} is currently not supported", n),
        }
    }

    pub fn all(self) -> impl Iterator<Item = usize> + 'a {
        let mut vals = IntegerStream::new(self.reg.value);
        core::iter::from_fn(move || match vals.remaining() {
            [] => None,
            _ => Some(match self.address_cells {
                1 => vals.next::<BigEndianU32>()?.get() as usize,
                2 => vals.next::<BigEndianU64>()?.get() as usize,
                n => panic!("address-cells of size {} is currently not supported", n),
            }),
        })
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
