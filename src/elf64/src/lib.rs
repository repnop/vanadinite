// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

use bytestream::{streamable_struct, ByteStream, FromBytes};

pub type Addr = u64;
pub type Off = u64;
pub type Half = u16;
pub type Word = u32;
pub type Sword = i32;
pub type Xword = u64;
pub type Sxword = i64;

pub const MACHINE_RISCV: Half = 243;
pub const FLAG_RISCV_RVC: Word = 0x0001;
pub const FLAG_RISCV_FLOAT_ABI_SOFT: Word = 0x0000;
pub const FLAG_RISCV_FLOAT_ABI_SINGLE: Word = 0x0002;
pub const FLAG_RISCV_FLOAT_ABI_DOUBLE: Word = 0x0004;
pub const FLAG_RISCV_FLOAT_ABI_QUAD: Word = 0x0006;
pub const FLAG_RISCV_FLOAT_ABI_MASK: Word = 0x0006;
pub const FLAG_RISCV_RVE: Word = 0x0008;
pub const FLAG_RISCV_TSO: Word = 0x0010;

#[derive(Debug, Clone, Copy)]
pub struct Elf<'a> {
    data: &'a [u8],
    pub header: Header,
}

impl<'a> Elf<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self { data, header: Header::from_bytes(data)? })
    }

    pub fn program_headers(&self) -> impl Iterator<Item = ProgramHeader> + '_ {
        let start = self.header.ph_offset as usize;
        let end = start + (self.header.ph_count as usize * core::mem::size_of::<ProgramHeader>());
        let mut phs = ByteStream::new(&self.data[start..end]);

        core::iter::from_fn(move || phs.next())
    }

    pub fn section_headers(&self) -> impl Iterator<Item = SectionHeader> + '_ {
        let start = self.header.sh_offset as usize;
        let end = start + (self.header.sh_count as usize * core::mem::size_of::<SectionHeader>());
        let mut phs = ByteStream::new(&self.data[start..end]);

        core::iter::from_fn(move || phs.next())
    }

    pub fn program_segment_data(&self, header: &ProgramHeader) -> &[u8] {
        &self.data[header.offset as usize..][..header.file_size as usize]
    }
}

streamable_struct! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct Header {
        pub ident: Identification,
        pub r#type: Half,
        pub machine: Half,
        pub version: Word,
        pub entry: Addr,
        pub ph_offset: Off,
        pub sh_offset: Off,
        pub flags: Word,
        pub eh_size: Half,
        pub ph_entry_size: Half,
        pub ph_count: Half,
        pub sh_entry_size: Half,
        pub sh_count: Half,
        pub sh_string_index: Half,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Identification {
    /// b"\x7FELF"
    pub magic: [u8; 4],
    pub class: u8,
    pub data: u8,
    pub version: u8,
    pub os_abi: u8,
    pub abi_version: u8,
}

impl FromBytes for Identification {
    const SIZE: usize = core::mem::size_of::<Self>() + 7;

    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < core::mem::size_of::<Header>() {
            return None;
        }

        match &data[..4] {
            [b'\x7F', b'E', b'L', b'F'] => Some(Self {
                magic: *b"\x7FELF",
                class: data[4],
                data: data[5],
                version: data[6],
                os_abi: data[7],
                abi_version: data[8],
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Class {
    ElfClass32 = 1,
    ElfClass64 = 2,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum DataEncoding {
    ElfData2Lsb = 1,
    ElfData2Msb = 2,
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum ObjectFileType {
    None = 0,
    Relocatable = 1,
    Executable = 2,
    SharedObject = 3,
    Core = 4,
    LoOs = 0xFE00,
    HiOs = 0xFEFF,
    LoProc = 0xFF00,
    HiProc = 0xFFFF,
}

streamable_struct! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct SectionHeader {
        /// Offset in bytes into the section name string table
        pub name: Word,
        /// Section type
        pub r#type: Word,
        // Section attributes
        pub flags: Xword,
        /// Virtual address of the beginning of the section, zero if not allocated
        pub addr: Addr,
        /// Offset in bytes to the section contents
        pub offset: Off,
        /// Size of the section in bytes
        pub size: Xword,
        /// Section index of an associated section
        pub link: Word,
        /// Extra section information
        pub info: Word,
        /// Required alignment of the section
        pub addr_align: Xword,
        /// Size in bytes of each section entry if the sizes are fixed, otherwise
        /// zero
        pub entry_size: Xword,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum SectionType {
    Null = 0,
    ProgBits = 1,
    SymbolTable = 2,
    StringTable = 3,
    Rela = 4,
    SymbolHashTable = 5,
    Dynamic = 6,
    Note = 7,
    NoBits = 8,
    Rel = 9,
    ShLib = 10,
    DynamicSymbolTable = 11,
    LoOs = 0x6000_0000,
    HiOs = 0x6FFF_FFFF,
    LoProc = 0x7000_0000,
    HiProc = 0x7FFF_FFFF,
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum SectionFlags {
    Write = 1,
    Alloc = 2,
    ExecInstr = 4,
    MaskOs = 0x0F00_0000,
    MaskProc = 0xF000_0000,
}

streamable_struct! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct SymbolTableEntry {
        pub name: Word,
        pub info: u8,
        pub _reserved: u8,
        pub section_table_index: Half,
        pub value: Addr,
        pub size: Xword,
    }
}

streamable_struct! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct ProgramHeader {
        pub r#type: Word,
        pub flags: Word,
        pub offset: Off,
        pub vaddr: Addr,
        pub paddr: Addr,
        pub file_size: Xword,
        pub memory_size: Xword,
        pub align: Xword,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum ProgramSegmentType {
    Null = 0,
    Load = 1,
    Dynamic = 2,
    Interpreter = 3,
    Note = 4,
    ShLib = 5,
    ProgramHeaderTable = 6,
    LoOs = 0x6000_0000,
    HiOs = 0x6FFF_FFFF,
    LoProc = 0x7000_0000,
    HiProc = 0x7FFF_FFFF,
}

impl core::cmp::PartialEq<ProgramSegmentType> for Word {
    fn eq(&self, other: &ProgramSegmentType) -> bool {
        *self == *other as Word
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum ProgramSegmentFlags {
    Executable = 1,
    Writeable = 2,
    Readable = 4,
    MaskOs = 0x00FF_0000,
    MaskProc = 0xFF00_0000,
}

// TODO: dynamic sections, hash table stuff
