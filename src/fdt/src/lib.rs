// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

mod cstr;
mod node;

use bytestream::{BigEndianU32, BigEndianU64, ByteStream, FromBytes};
pub use node::{Compatible, MappedArea, MemoryNode, MemoryRegion, NodeProperty};

#[derive(Debug)]
pub enum FdtError {
    BadMagic,
    BadPtr,
    BufferTooSmall,
}

impl core::fmt::Display for FdtError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FdtError::BadMagic => write!(f, "bad FDT magic value"),
            FdtError::BadPtr => write!(f, "an invalid pointer was passed"),
            FdtError::BufferTooSmall => write!(f, "the given buffer was too small to contain a FDT header"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Fdt<'a> {
    data: &'a [u8],
    header: FdtHeader,
}

bytestream::streamable_struct! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct FdtHeader {
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
}

impl FdtHeader {
    fn valid_magic(&self) -> bool {
        self.magic.get() == 0xd00dfeed
    }

    fn struct_range(&self) -> core::ops::Range<usize> {
        let start = self.off_dt_struct.get() as usize;
        let end = start + self.size_dt_struct.get() as usize;

        start..end
    }

    fn strings_range(&self) -> core::ops::Range<usize> {
        let start = self.off_dt_strings.get() as usize;
        let end = start + self.size_dt_strings.get() as usize;

        start..end
    }
}

impl<'a> Fdt<'a> {
    /// # Safety
    /// This function checks the pointer alignment and performs a read to verify
    /// the magic value. If the pointer is invalid this can result in undefined
    /// behavior.
    pub unsafe fn from_ptr(ptr: *const u8) -> Result<Self, FdtError> {
        if ptr.is_null() {
            return Err(FdtError::BadPtr);
        }

        let tmp_header = core::slice::from_raw_parts(ptr, core::mem::size_of::<FdtHeader>());
        let real_size = Self::new(tmp_header)?.header.totalsize.get() as usize;

        Self::new(core::slice::from_raw_parts(ptr, real_size))
    }

    pub fn new(data: &'a [u8]) -> Result<Self, FdtError> {
        let mut stream = ByteStream::new(data);
        let header: FdtHeader = stream.next().ok_or(FdtError::BufferTooSmall)?;

        if !header.valid_magic() {
            return Err(FdtError::BadMagic);
        }

        Ok(Self { data, header })
    }

    pub fn strings(&self) -> impl Iterator<Item = &'a str> {
        let mut block = self.strings_block();

        core::iter::from_fn(move || {
            if block.is_empty() {
                return None;
            }

            let cstr = cstr::CStr::new(block);

            block = &block[cstr.len() + 1..];

            cstr.as_str()
        })
    }

    pub fn memory_reservations(&self) -> impl Iterator<Item = MemoryReservation> + 'a {
        let mut stream = ByteStream::new(&self.data[self.header.off_mem_rsvmap.get() as usize..]);
        let mut done = false;

        core::iter::from_fn(move || {
            if stream.is_empty() || done {
                return None;
            }

            let res = stream.next::<MemoryReservation>()?;

            if res.address.get() == 0 && res.size.get() == 0 {
                done = true;
                return None;
            }

            Some(res)
        })
    }

    pub fn root(&self) -> Root<'_, 'a> {
        Root { node: self.find_node("/").expect("/ is a required node") }
    }

    pub fn aliases(&self) -> Aliases<'_, 'a> {
        Aliases { node: self.find_node("/aliases").expect("/aliases is a required node"), header: self }
    }

    pub fn cpus(&self) -> impl Iterator<Item = Cpu<'_, 'a>> {
        let parent = self.find_node("/cpus").expect("/cpus is a required node");

        parent
            .children()
            .filter(|c| c.name.split('@').next().unwrap() == "cpu")
            .map(move |cpu| Cpu { parent, node: cpu })
    }

    /// Returns the first node that matches the node path, if you want all that
    /// match the path, use `find_all_nodes`. This will automatically attempt to
    /// resolve aliases if `path` is not found.
    pub fn find_node(&self, path: &str) -> Option<node::FdtNode<'_, 'a>> {
        let node = node::find_node(&mut ByteStream::new(self.structs_block()), path, self, None);
        node.or_else(|| self.aliases().resolve_node(path))
    }

    pub fn find_all_nodes(&self, path: &'a str) -> impl Iterator<Item = node::FdtNode<'_, 'a>> {
        let mut done = false;
        let only_root = path == "/";
        let valid_path = path.chars().fold(0, |acc, c| acc + if c == '/' { 1 } else { 0 }) >= 1;

        let mut path_split = path.rsplitn(2, '/');
        let child_name = path_split.next().unwrap();
        let parent_path = match path_split.next().unwrap() {
            "" => "/",
            s => s,
        };
        let parent = node::find_node(&mut ByteStream::new(self.structs_block()), parent_path, self, None);
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

    pub fn all_nodes(&self) -> impl Iterator<Item = node::FdtNode<'_, 'a>> {
        node::all_nodes(self)
    }

    pub fn find_phandle(&self, phandle: u32) -> Option<node::FdtNode<'_, 'a>> {
        self.all_nodes().find(|n| {
            n.properties()
                .find(|p| p.name == "phandle")
                .and_then(|p| Some(BigEndianU32::from_bytes(p.value)?.get() == phandle))
                .unwrap_or(false)
        })
    }

    pub fn chosen(&self) -> Option<Chosen<'_, 'a>> {
        node::find_node(&mut ByteStream::new(self.structs_block()), "/chosen", self, None).map(|node| Chosen { node })
    }

    pub fn find_compatible(&self, with: &[&str]) -> Option<node::FdtNode<'_, 'a>> {
        self.all_nodes()
            .find(|n| n.compatible().and_then(|compats| compats.all().find(|c| with.contains(&c))).is_some())
    }

    pub fn memory(&self) -> MemoryNode<'_, 'a> {
        MemoryNode { node: self.find_node("/memory").expect("requires memory node") }
    }

    pub fn total_size(&self) -> usize {
        self.header.totalsize.get() as usize
    }

    fn cstr_at_offset(&self, offset: usize) -> cstr::CStr<'a> {
        cstr::CStr::new(&self.strings_block()[offset..])
    }

    fn str_at_offset(&self, offset: usize) -> &'a str {
        self.cstr_at_offset(offset).as_str().expect("not utf-8 cstr")
    }

    fn strings_block(&self) -> &'a [u8] {
        &self.data[self.header.strings_range()]
    }

    fn structs_block(&self) -> &'a [u8] {
        &self.data[self.header.struct_range()]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Chosen<'b, 'a: 'b> {
    node: node::FdtNode<'b, 'a>,
}

impl<'b, 'a: 'b> Chosen<'b, 'a> {
    pub fn bootargs(self) -> Option<&'a str> {
        self.node
            .properties()
            .find(|n| n.name == "bootargs")
            .and_then(|n| core::str::from_utf8(&n.value[..n.value.len() - 1]).ok())
    }

    pub fn stdout(self) -> Option<node::FdtNode<'b, 'a>> {
        self.node
            .properties()
            .find(|n| n.name == "stdout-path")
            .and_then(|n| core::str::from_utf8(&n.value[..n.value.len() - 1]).ok())
            .and_then(|name| self.node.header.find_node(name))
    }

    pub fn stdin(self) -> Option<node::FdtNode<'b, 'a>> {
        self.node
            .properties()
            .find(|n| n.name == "stdin-path")
            .and_then(|n| core::str::from_utf8(&n.value[..n.value.len() - 1]).ok())
            .and_then(|name| self.node.header.find_node(name))
            .or_else(|| self.stdout())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Root<'b, 'a: 'b> {
    node: node::FdtNode<'b, 'a>,
}

impl<'b, 'a: 'b> Root<'b, 'a> {
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
pub struct Aliases<'b, 'a: 'b> {
    header: &'b Fdt<'a>,
    node: node::FdtNode<'b, 'a>,
}

impl<'b, 'a: 'b> Aliases<'b, 'a> {
    pub fn resolve(self, alias: &str) -> Option<&'a str> {
        self.node.properties().find(|p| p.name == alias).and_then(|p| core::str::from_utf8(p.value).ok())
    }

    pub fn resolve_node(self, alias: &str) -> Option<node::FdtNode<'b, 'a>> {
        self.resolve(alias).and_then(|name| self.header.find_node(name))
    }

    pub fn all(self) -> impl Iterator<Item = (&'a str, &'a str)> + 'b {
        self.node.properties().filter_map(|p| Some((p.name, core::str::from_utf8(p.value).ok()?)))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cpu<'b, 'a: 'b> {
    parent: node::FdtNode<'b, 'a>,
    node: node::FdtNode<'b, 'a>,
}

impl<'b, 'a: 'b> Cpu<'b, 'a> {
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

    pub fn properties(self) -> impl Iterator<Item = NodeProperty<'a>> + 'b {
        self.node.properties()
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
        let mut vals = ByteStream::new(self.reg.value);
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

bytestream::streamable_struct! {
    #[derive(Debug)]
    #[repr(C)]
    pub struct MemoryReservation {
        address: BigEndianU64,
        size: BigEndianU64,
    }
}

impl MemoryReservation {
    pub fn address(&self) -> *const u8 {
        self.address.get() as usize as *const u8
    }

    pub fn size(&self) -> usize {
        self.size.get() as usize
    }
}

#[cfg(test)]
mod tests;
