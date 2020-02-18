//! # Flattened Devicetree v0.3 module
//!
//! Implementation of the Flattened Devicetree (FDT) Devicetree Blob (DTB)
//! format (v0.3 specification) which allows detection of the various parts of
//! the system (RAM zones, MMIO, VirtIO, etc).
//!

use crate::util::{self, CStr, PtrUtils};

#[derive(Debug, Clone)]
pub struct Fdt {
    base: *const u8,
    offset: usize,
}

impl Fdt {
    pub unsafe fn new(base: *const u8) -> Self {
        base.assert_aligned_to::<u32>();
        Self { base, offset: 0 }
    }

    pub fn header(&self) -> FdtHeader {
        unsafe { FdtHeader::from_ptr(self.base) }
    }

    pub fn reserved_memory_blocks(&self) -> impl Iterator<Item = FdtReserveEntry> {
        let header = self.header();
        let mut ptr = unsafe { self.base.add(header.off_mem_rsvmap as usize) };
        let mut done = false;
        core::iter::from_fn(move || {
            if !done {
                ptr.assert_aligned_to::<u64>();
                let address = unsafe { u64::from_be(ptr.cast::<u64>().read_volatile()) };
                ptr = unsafe { ptr.add(core::mem::size_of::<u64>()) };

                let size = unsafe { u64::from_be(ptr.cast::<u64>().read_volatile()) };
                ptr = unsafe { ptr.add(core::mem::size_of::<u64>()) };

                if address == 0 && size == 0 {
                    done = true;
                    return None;
                }

                return Some(FdtReserveEntry { address, size });
            }

            None
        })
    }

    // pub fn structure_nodes(&self) -> impl Iterator<Item = FdtStructureNode> {
    //     let header = self.header();
    //     let mut struct_ptr = unsafe { self.base.add(header.off_dt_struct as usize).cast::<u32>() };
    //     let end_ptr = unsafe {
    //         struct_ptr
    //             .cast::<u8>()
    //             .add(header.size_dt_struct as usize)
    //             .cast()
    //     };
    //     let mut done = false;
    //
    //     struct_ptr.assert_aligned_to_self();
    //
    //     core::iter::from_fn(move || loop {
    //         if done {
    //             return None;
    //         }
    //
    //         let token = unsafe { advance_token(&mut struct_ptr) };
    //         let token = token.expect("not an FDT token");
    //         log::debug!("{:?}", token);
    //         match token {
    //             FdtToken::BeginNode => {
    //                 let name = unsafe { CStr::new(struct_ptr.cast()) };
    //                 log::debug!("{}", name);
    //                 let first_prop =
    //                     unsafe { struct_ptr.add((name.len() + 1) / 4).align_up_to_self() };
    //                 struct_ptr = first_prop;
    //                 log::debug!("yes");
    //                 unsafe {
    //                     advance_to_end_of_node(&mut struct_ptr, end_ptr);
    //                 }
    //
    //                 return Some(FdtStructureNode { name, first_prop });
    //             }
    //             FdtToken::End => {
    //                 done = true;
    //                 return None;
    //             }
    //             FdtToken::Nop => continue,
    //             _ => unreachable!("Got FDT token: {:?}", token),
    //         }
    //     })
    // }

    pub unsafe fn print_structure_block(&self) {
        let header = self.header();
        let mut struct_ptr = self.base.add(header.off_dt_struct as usize).cast::<u32>();

        for i in 0..8 {
            log::debug!("{}", util::DebugBytesAt::new(struct_ptr.add(16 * i).cast()));
        }

        log::debug!("------");

        loop {
            log::debug!("{}", util::DebugBytesAt::new(struct_ptr.cast()));
            let token = advance_token(&mut struct_ptr).expect("token");

            match token {
                FdtToken::BeginNode => {
                    let name = CStr::new(struct_ptr.cast());
                    log::debug!("BeginNode: {}", name);

                    let mut after_name = struct_ptr.cast::<u8>().add(name.len() + 1);

                    while after_name as usize % 4 != 0 {
                        after_name = after_name.add(1);
                    }

                    struct_ptr = after_name.cast();
                    struct_ptr.assert_aligned_to_self();
                }
                FdtToken::Prop => {
                    let prop = FdtNodeProp::from_ptr(struct_ptr);
                    struct_ptr = struct_ptr.add(2);

                    let name = CStr::new(
                        self.base
                            .add(header.off_dt_strings as usize)
                            .add(prop.nameoff as usize),
                    );
                    log::debug!("Prop:");
                    log::debug!("    Name: {}", name);
                    log::debug!(
                        "    Value: {:?}",
                        core::str::from_utf8(core::slice::from_raw_parts(
                            struct_ptr.cast::<u8>(),
                            prop.len as usize
                        ))
                        .unwrap()
                    );
                }
                FdtToken::EndNode => {
                    log::debug!("EndNode");
                }
                FdtToken::End => {
                    log::debug!("End");
                }
                _ => todo!("{:?}", token),
            }
        }
    }

    pub unsafe fn print_strings(&self) {
        let header = self.header();
        let end = self
            .base
            .add(header.off_dt_strings as usize)
            .add(header.size_dt_strings as usize);
        let mut strings_ptr = self.base.add(header.off_dt_strings as usize);

        while (strings_ptr as usize) < end as usize {
            let s = CStr::new(strings_ptr);
            log::debug!("String: {}", s);
            strings_ptr = strings_ptr.add(s.len() + 1);
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
enum FdtToken {
    BeginNode = 1,
    EndNode = 2,
    Prop = 3,
    Nop = 4,
    End = 9,
}

unsafe fn advance_token(ptr: &mut *const u32) -> Option<FdtToken> {
    ptr.assert_aligned_to::<u32>();
    let n = u32::from_be(ptr.read());
    *ptr = ptr.add(1);

    match n {
        1 => Some(FdtToken::BeginNode),
        2 => Some(FdtToken::EndNode),
        3 => Some(FdtToken::Prop),
        4 => Some(FdtToken::Nop),
        9 => Some(FdtToken::End),
        _ => {
            log::debug!("Non-FDT token value: {:#x}", n);
            None
        }
    }
}

unsafe fn advance_to_end_of_node(ptr: &mut *const u32, struct_end_ptr: *const u32) {
    ptr.assert_aligned_to::<u32>();

    loop {
        assert!(*ptr < struct_end_ptr);
        let token = advance_token(ptr).expect("bad token");

        match token {
            FdtToken::EndNode => break,
            FdtToken::Prop => {
                let prop = FdtNodeProp::from_ptr(*ptr);
                *ptr = ptr
                    .add(2)
                    .cast::<u8>()
                    .add(prop.len as usize)
                    .cast::<u32>()
                    .align_up_to_self();
            }
            FdtToken::Nop => continue,
            _ => unreachable!(),
        }
    }
}

pub struct FdtStructureNode {
    name: CStr,
    first_prop: *const u32,
}

impl FdtStructureNode {
    pub fn name(&self) -> &CStr {
        &self.name
    }
    //pub fn properties(&self) -> impl Iterator<Item = FdtStructureNodeProperty> {}
}

struct FdtNodeProp {
    len: u32,
    nameoff: u32,
}

impl FdtNodeProp {
    unsafe fn from_ptr(mut ptr: *const u32) -> Self {
        let len = u32::from_be(ptr.read());
        ptr = ptr.add(1);
        let nameoff = u32::from_be(ptr.read());

        Self { len, nameoff }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FdtReserveEntry {
    pub address: u64,
    pub size: u64,
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
