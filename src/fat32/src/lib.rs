// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

#[cfg(test)]
extern crate std;

use common::{
    byteorder::IntegerStream,
    io::{Block, BlockDevice, Offset},
    stream_ints,
};

#[derive(Debug)]
pub struct Fat32<B: BlockDevice> {
    device: B,
    starting_block: Block,
    bpb: BiosParameterBlock,
    fs_info: FsInfo,
}

impl<B: BlockDevice> Fat32<B> {
    pub fn new(mut device: B, starting_block: Block) -> Result<Self, B::Error> {
        let mut bpb_bytes = [0; 512];
        device.read(starting_block, Offset(0), &mut bpb_bytes[..])?;

        let mut stream = IntegerStream::new(&bpb_bytes[..]);

        stream_ints!(stream, {
            skip 0x0B bytes,

            let bytes_per_sector: u16,
            let sectors_per_cluster: u8,
            let num_reserved_sectors: u16,
            let num_fats: u8,

            skip 0x13 bytes,

            let sectors_per_fat: u32,
            let root_directory_first_cluster: u32,
        });

        let sig_bytes = [bpb_bytes[0x1FE], bpb_bytes[0x1FF]];
        let signature = u16::from_ne_bytes(sig_bytes);

        assert_eq!(bytes_per_sector, 512);
        assert!(sectors_per_cluster.is_power_of_two() && sectors_per_cluster <= 128);
        assert_eq!(num_fats, 2);
        assert_eq!(signature, 0xAA55);

        todo!()

        //let bpb = VolumeId {
        //    bytes_per_sector,
        //    sectors_per_cluster,
        //    num_reserved_sectors,
        //    num_fats,
        //    sectors_per_fat,
        //    root_directory_first_cluster,
        //};
        //
        //Ok(Self { device, starting_block, bpb })
    }

    fn fat_begin_lba(&self) -> (Block, Offset) {
        if self.device.block_size() == 512 {
            // Happy path
            (Block(self.starting_block.0 + self.bpb.num_reserved_sectors as usize), Offset(0))
        } else {
            let reserved_sector_bytes = 512 * self.bpb.num_reserved_sectors as usize;
            let block_byte_offset = self.starting_block.0 * self.device.block_size();
            let byte_pos = block_byte_offset + reserved_sector_bytes;
            let (block, offset) = self.device.block_and_offset(byte_pos);

            (self.starting_block + block, offset)
        }
    }

    fn cluster_begin_lba(&self) -> (Block, Offset) {
        if self.device.block_size() == 512 {
            // Happy path
            let block = self.starting_block.0
                + self.bpb.num_reserved_sectors as usize
                + (self.bpb.num_fats as usize * self.bpb.sectors_per_fat as usize);

            (Block(block), Offset(0))
        } else {
            let reserved_sector_bytes = 512 * self.bpb.num_reserved_sectors as usize;
            let block_byte_offset = self.starting_block.0 * self.device.block_size();
            let byte_pos = block_byte_offset
                + reserved_sector_bytes
                + (512 * self.bpb.num_fats as usize * self.bpb.sectors_per_fat as usize);
            let (block, offset) = self.device.block_and_offset(byte_pos);

            (self.starting_block + block, offset)
        }
    }

    fn sectors_per_cluster(&self) -> usize {
        self.bpb.sectors_per_cluster as usize
    }

    fn root_directory_first_cluster(&self) -> usize {
        self.bpb.root_cluster as usize
    }

    fn cluster_block_and_offset(&self, cluster: usize) -> (Block, Offset) {
        assert!(cluster >= 2, "bad cluster value: {}", cluster);

        if self.device.block_size() == 512 {
            (Block((self.cluster_begin_lba().0).0 + (cluster - 2) * self.sectors_per_cluster()), Offset(0))
        } else {
            let (block, offset) = self.cluster_begin_lba();
            let cluster_byte_offset = (cluster - 2) * 512 * self.bpb.sectors_per_cluster as usize;
            let bytes = block.to_bytes(&self.device, offset) + cluster_byte_offset;

            self.device.block_and_offset(bytes)
        }
    }
}

#[derive(Debug)]
struct BiosParameterBlock {
    // Offset: 0x0B
    /// Number of bytes per sector, usually 512 but not required to be
    bytes_per_sector: u16,

    // Offset: 0x0D
    /// Number of sectors per cluster
    sectors_per_cluster: u8,

    // Offset: 0x0E
    /// Number of reserved sectors after the BPB
    num_reserved_sectors: u16,

    // Offset: 0x10
    /// Number of FATs, usually 2 but not required to be
    num_fats: u8,

    // Root directory could would be next but its all zeroes for FAT32.
    // Similarly with the Total Sector count, the Media byte, and all stuff up
    // to the total sector count. We also ignore any of the BS_* fields since
    // they don't really have any information we're interested in

    // Offset: 0x20
    /// Total number of sectors of the storage
    total_sectors: u32,

    // Offset: 0x24
    /// Sectors per FAT
    sectors_per_fat: u32,

    // Offset: 0x28
    /// Flags for determining FAT mirroring etc, currently ignored
    ext_flags: u16,

    // Offset: 0x30
    /// File system version in the format `minor:major` where `minor` is byte 0
    /// and `major` is byte 1.
    fs_version: [u8; 2],

    // Offset: 0x32
    /// Cluster number of the root cluster
    root_cluster: u32,

    // Offset: 0x36
    /// FSInfo sector number, usually 1
    fs_info_sector: u16,
}

#[derive(Debug)]
struct FsInfo {
    // Offset: 0x00
    // MUST contain the value 0x41615252
    /// Leading signature to validate this is a FSInfo structure
    lead_signature: u32,

    // Offset: 0x1E4
    // MUST contain the value 0x61417272
    /// Another verification signature
    struct_signature: u32,

    // Offset: 0x1E8
    /// Last known free cluster count, if unknown has the value `0xFFFFFFFF`
    free_count: u32,

    // Offset: 0x1EC
    /// Hint for the next free cluster, if unknown has the value `0xFFFFFFFF`
    /// and must be calculated by starting at cluster 2
    next_free: u32,
}

pub struct DirectoryRecord {
    // Offset: 0x00
    /// Short filename for the directory entry
    short_filename: [u8; 11],
    // Offset: 0x0B
    /// Attribute byte
    attribute: u8,
    /// Reserved byte
    _res: u8,
    // Offset: 0x0D
    /// Millisecond stamp at file creation time, valid values are [0, 199]
    creation_tenth_sec: u8,
    // Offset: 0x14
    /// High word of the entry's first cluster number
    first_cluster_high: u16,
    // Offset: 0x16
    modify_time: ModifyTime,
    modify_date: ModifyDate,
    // Offset: 0x1A
    /// High word of the entry's first cluster number
    first_cluster_low: u16,
    file_size: u32,
}

impl DirectoryRecord {
    pub fn read_only(&self) -> bool {
        self.attribute & 0b0001 == 0b0001
    }

    pub fn hidden(&self) -> bool {
        self.attribute & 0b0010 == 0b0010
    }

    pub fn system(&self) -> bool {
        self.attribute & 0b0100 == 0b0100
    }

    pub fn bpb(&self) -> bool {
        self.attribute & 0b1000 == 0b1000
    }

    pub fn long_filename(&self) -> bool {
        self.attribute & 0b1111 == 0b1111
    }

    pub fn subdirectory(&self) -> bool {
        self.attribute & 0b10000 == 0b10000
    }

    pub fn archive(&self) -> bool {
        self.attribute & 0b100000 == 0b100000
    }

    pub fn unused(&self) -> bool {
        self.short_filename[0] == 0xE5
    }

    pub fn end_of_directory(&self) -> bool {
        self.short_filename[0] == 0x00
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ModifyTime(u16);

impl ModifyTime {
    pub fn second(self) -> u8 {
        ((self.0 & 0x001F) as u8) * 2
    }

    pub fn minute(self) -> u8 {
        ((self.0 & 0b0000_0111_1110_0000) >> 5) as u8
    }

    pub fn hour(self) -> u8 {
        ((self.0 & 0b1111_1000_0000_0000) >> 11) as u8
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ModifyDate(u16);

impl ModifyDate {
    pub fn day(self) -> u8 {
        (self.0 & 0x001F) as u8
    }

    pub fn month(self) -> u8 {
        (self.0 & 0b0000_0001_1110_0000 >> 5) as u8
    }

    pub fn year(self) -> u16 {
        (self.0 & 0b1111_1110_0000_0000 >> 9) + 1980
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockDrive {
        backing: std::fs::File,
    }

    impl MockDrive {
        fn new(backing: std::fs::File) -> Self {
            Self { backing }
        }
    }

    impl super::BlockDevice for MockDrive {
        type Error = std::io::Error;

        fn block_size(&self) -> usize {
            512
        }

        fn num_blocks(&self) -> Option<usize> {
            None
        }

        fn read(
            &mut self,
            start: common::io::Block,
            offset: common::io::Offset,
            buf: &mut [u8],
        ) -> Result<(), Self::Error> {
            use std::io::{Read, Seek, SeekFrom};

            let bytes = start.0 * 512 + offset.0;

            self.backing.seek(SeekFrom::Start(bytes as u64))?;
            self.backing.read_exact(buf)?;

            Ok(())
        }

        fn write(
            &mut self,
            start: common::io::Block,
            offset: common::io::Offset,
            buf: &[u8],
        ) -> Result<(), Self::Error> {
            use std::io::{Seek, SeekFrom, Write};

            let bytes = start.0 * 512 + offset.0;

            self.backing.seek(SeekFrom::Start(bytes as u64))?;
            self.backing.write_all(buf)?;

            Ok(())
        }
    }

    #[test]
    fn heck() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        let mock_drive = MockDrive::new(std::fs::File::open("../test_fat.fs")?);
        let fat = Fat32::new(mock_drive, Block(0))?;

        Ok(())
    }
}
