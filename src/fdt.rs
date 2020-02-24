//! # Flattened Devicetree v0.3 module
//!
//! Implementation of the Flattened Devicetree (FDT) Devicetree Blob (DTB)
//! format (v0.3 specification) which allows detection of the various parts of
//! the system (RAM zones, MMIO, VirtIO, etc).
//!
//! More info on the usage of devicetrees: https://elinux.org/Device_Tree_Usage

use crate::util::{CStr, PtrUtils};
use tinyvec::ArrayVec;

pub struct Node<'a> {
    fdt: &'a Fdt,
    index: usize,
}

impl Node<'_> {
    pub fn properties(&self) -> &[FdtProperty] {
        &self.fdt.nodes[self.index].properties
    }

    pub fn name(&self) -> &str {
        self.fdt.nodes[self.index]
            .name
            .as_str()
            .unwrap()
            .split('@')
            .next()
            .unwrap()
    }

    pub fn address(&self) -> Option<usize> {
        Some(
            usize::from_str_radix(
                self.fdt.nodes[self.index]
                    .name
                    .as_str()
                    .unwrap()
                    .split('@')
                    .nth(1)?,
                16,
            )
            .ok()?,
        )
    }
}

impl core::ops::Index<&'_ str> for Node<'_> {
    type Output = FdtProperty;

    fn index(&self, idx: &str) -> &FdtProperty {
        self.properties().iter().find(|p| p.name() == idx).unwrap()
    }
}

#[derive(Debug)]
pub struct Fdt {
    header: FdtHeader,
    nodes: ArrayVec<[FdtNode; 32]>,
    reserved_memory_blocks: ArrayVec<[FdtReserveEntry; 32]>,
}

impl Fdt {
    pub unsafe fn from_ptr(base: *const u8) -> Self {
        let header = FdtHeader::from_ptr(base);
        let mut nodes = ArrayVec::new();
        let mut reserved_memory_blocks = ArrayVec::new();

        // Get reserved memory blocks
        let mut rmb_ptr = base.add(header.off_mem_rsvmap as usize).cast::<u64>();
        rmb_ptr.assert_aligned_to_self();

        loop {
            let address = u64::from_be(rmb_ptr.read_and_increment());
            let size = u64::from_be(rmb_ptr.read_and_increment());

            match (address, size) {
                (0, 0) => break,
                (address, size) => reserved_memory_blocks.push(FdtReserveEntry { address, size }),
            }
        }

        // Get nodes
        nodes.push(FdtNode::default());
        get_nodes(
            base.add(header.off_dt_strings as usize),
            &mut nodes,
            0,
            &mut base.add(header.off_dt_struct as usize).cast(),
        );

        Self {
            header,
            nodes,
            reserved_memory_blocks,
        }
    }

    pub fn root(&self) -> Node<'_> {
        Node {
            fdt: self,
            index: 0,
        }
    }

    pub fn find(&self, name: &str) -> Option<Node<'_>> {
        for (index, node) in self.nodes.iter().enumerate() {
            if node.name.as_str().unwrap().split('@').next().unwrap() == name {
                return Some(Node { fdt: self, index });
            }
        }

        None
    }
}

// This is implemented as a function for recursion which makes getting all of
// the nodes added a breeze
unsafe fn get_nodes(
    string_base: *const u8,
    nodes: &mut ArrayVec<[FdtNode; 32]>,
    current_node: usize,
    ptr: &mut *const u32,
) {
    const BEGIN_NODE: u32 = 1;
    const END_NODE: u32 = 2;
    const PROP: u32 = 3;
    const NOP: u32 = 4;
    const END: u32 = 9;

    ptr.assert_aligned_to_self();

    // TODO: check for nops and stuff
    assert_eq!(u32::from_be(ptr.read_and_increment()), BEGIN_NODE);

    let name = CStr::new(ptr.cast());
    let len = name.len();
    nodes[current_node].name = name;
    *ptr = ptr.cast::<u8>().add(len + 1).align_up_to::<u32>().cast();

    while u32::from_be(ptr.read()) != END_NODE {
        match u32::from_be(ptr.read()) {
            BEGIN_NODE => {
                nodes.push(FdtNode::default());
                let index = nodes.len() - 1;
                get_nodes(string_base, nodes, index, ptr);
                nodes[current_node].child_nodes.push(index);
            }
            PROP => {
                ptr.read_and_increment();
                let len = u32::from_be(ptr.read_and_increment()) as usize;
                let nameoff = u32::from_be(ptr.read_and_increment());

                nodes[current_node].properties.push(FdtProperty {
                    name: CStr::new(string_base.add(nameoff as usize)),
                    value: core::slice::from_raw_parts(ptr.cast(), len),
                });

                *ptr = ptr.cast::<u8>().add(len).align_up_to::<u32>().cast();
            }
            NOP => {
                ptr.read_and_increment();
                continue;
            }
            n => todo!("error handling: {}", n),
        }
    }

    // TODO: check for nops and stuff
    assert_eq!(u32::from_be(ptr.read_and_increment()), END_NODE);
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct FdtReserveEntry {
    pub address: u64,
    pub size: u64,
}

#[derive(Debug, Default)]
pub struct FdtNode {
    name: CStr,
    properties: ArrayVec<[FdtProperty; 32]>,
    child_nodes: ArrayVec<[usize; 32]>,
}

#[derive(Debug)]
pub struct FdtProperty {
    name: CStr,
    value: &'static [u8],
}

impl FdtProperty {
    pub fn name(&self) -> &str {
        self.name.as_str().unwrap()
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

impl Default for FdtProperty {
    fn default() -> Self {
        static EMPTY_VALUE: &[u8] = &[];

        Self {
            name: Default::default(),
            value: EMPTY_VALUE,
        }
    }
}

/// FDT header structure, describes the following devicetree
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FdtHeader {
    /// Must be `0xd00dfeed` when in big-endian format
    pub magic: u32,
    /// Total size in bytes of the DTB
    pub total_size: u32,
    /// Offset in bytes of the structure block
    pub off_dt_struct: u32,
    /// Offset in bytes of the strings block
    pub off_dt_strings: u32,
    /// Offset in bytes of the memory reservation block
    pub off_mem_rsvmap: u32,
    /// Version of the spec DTB uses, 17 (0x11) for v0.3
    pub version: u32,
    /// Lowest version this structure is backwards compatibile with
    pub last_comp_version: u32,
    /// Physical ID of the boot CPU
    pub boot_cpuid_phys: u32,
    /// Size in bytes of the strings block
    pub size_dt_strings: u32,
    /// Size in bytes of the struct block
    pub size_dt_struct: u32,
}

impl FdtHeader {
    unsafe fn from_ptr(mut ptr: *const u8) -> Self {
        ptr.assert_aligned_to::<u32>();

        let mut read_be_u32 = || {
            let n = ptr.cast::<u32>().read_volatile();
            ptr = ptr.add(core::mem::size_of::<u32>());

            u32::from_be(n)
        };

        let magic = read_be_u32();
        assert_eq!(
            magic, 0xd00d_feed,
            "assert: incorrect FDT header! probably not reading the correct memory"
        );

        Self {
            magic,
            total_size: read_be_u32(),
            off_dt_struct: read_be_u32(),
            off_dt_strings: read_be_u32(),
            off_mem_rsvmap: read_be_u32(),
            version: read_be_u32(),
            last_comp_version: read_be_u32(),
            boot_cpuid_phys: read_be_u32(),
            size_dt_strings: read_be_u32(),
            size_dt_struct: read_be_u32(),
        }
    }
}
