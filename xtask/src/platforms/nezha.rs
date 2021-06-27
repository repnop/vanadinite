use anyhow::Context;
use bytemuck::{Pod, Zeroable};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

const SDMMC_BOOT0_SECTOR_OFFSET: usize = 0x10;
const SDMMC_TOC_SECTOR_OFFSET: usize = 32800;
const SECTOR_SIZE: usize = 512;
const TOC_HEADER_CHKSUM_STAMP: u32 = 0x5F0A6C39;
const TOC_HEADER_MAGIC: u32 = 0x89119800;
const TOC_HEADER_END_MAGIC: u32 = 0x3B45494D;
const TOC_ENTRY_END_MAGIC: u32 = 0x3B454949;
const OPENSBI_ENTRY_NAME: [u8; 64] = *b"opensbi\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
const OPENSBI_RUN_ADDR: u32 = 0x42000000;
const DTB_ENTRY_NAME: [u8; 64] = *b"dtb\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
const DTB_LOAD_ADDR: u32 = 0x41000000;
const UBOOT_ENTRY_NAME: [u8; 64] = *b"u-boot\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
const UBOOT_LOAD_ADDR: u32 = DTB_LOAD_ADDR - (2 * 1024 * 1024);

static ZEROES: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];

pub fn generate_sdcard_image(write_to: &Path, boot0_bin: &Path, opensbi_bin: &Path, dtb: &Path) -> anyhow::Result<()> {
    let boot0_bin = fs::read(boot0_bin).context("failed to read boot0 binary")?;

    // We need to patch the OpenSBI image so it knows where the devicetree is...
    // yikes. We place it sufficiently far enough into memory that it shouldn't
    // conflict with the kernel as its kinda large.
    let mut opensbi_bin = fs::read(opensbi_bin).context("failed to read OpenSBI binary")?;
    opensbi_bin[12..16].copy_from_slice(&DTB_LOAD_ADDR.to_le_bytes());

    let dtb = fs::read(dtb).context("failed to read DTB file")?;
    let mut img = File::create(write_to).context("failed to create sdcard image file")?;

    for _ in 0..SDMMC_BOOT0_SECTOR_OFFSET {
        img.write_all(&ZEROES).context("failed to zero pre-boot0 sector")?;
    }

    img.write_all(&boot0_bin).unwrap();

    let until_toc =
        (SDMMC_TOC_SECTOR_OFFSET * SECTOR_SIZE) - (SDMMC_BOOT0_SECTOR_OFFSET * SECTOR_SIZE + boot0_bin.len());

    for _ in 0..(until_toc / 512) {
        img.write_all(&ZEROES).context("failed to zero post-boot0 sector")?;
    }

    for _ in 0..(until_toc % 512) {
        img.write_all(&[0]).context("failed to zero post-boot0 sector")?;
    }

    //for _ in 0..SDMMC_TOC_SECTOR_OFFSET {
    //    img.write_all(&ZEROES).context("failed to zero start of image")?;
    //}

    let opensbi_entry = SbromToc1ItemInfo {
        name: OPENSBI_ENTRY_NAME,
        data_offset: 2048,
        data_len: opensbi_bin.len() as u32,
        encrypt: 0,
        type_: 3,
        run_addr: OPENSBI_RUN_ADDR,
        index: 0,
        _reserved: [0; 69],
        end: TOC_ENTRY_END_MAGIC,
    };

    let opensbi_end = opensbi_entry.data_offset + opensbi_entry.data_len;
    let padding = 0x200 - (opensbi_end & 0x1FF);

    // We need a dummy U-Boot entry so it'll actually place the devicetree at
    // the right spot in memory..
    let uboot_entry = SbromToc1ItemInfo {
        name: UBOOT_ENTRY_NAME,
        data_offset: opensbi_end + padding,
        data_len: 512,
        encrypt: 0,
        type_: 3,
        run_addr: UBOOT_LOAD_ADDR,
        index: 0,
        _reserved: [0; 69],
        end: TOC_ENTRY_END_MAGIC,
    };

    let dtb_entry = SbromToc1ItemInfo {
        name: DTB_ENTRY_NAME,
        data_offset: opensbi_end + padding,
        data_len: dtb.len() as u32,
        encrypt: 0,
        type_: 3,
        run_addr: 0,
        index: 0,
        _reserved: [0; 69],
        end: TOC_ENTRY_END_MAGIC,
    };

    let dtb_end = dtb_entry.data_offset + dtb_entry.data_len;
    let end_padding = 0x200 - (dtb_end & 0x1FF);

    let header = SbromToc1HeaderInfo {
        name: *b"vanadinite-pkg\0\0",
        magic: TOC_HEADER_MAGIC,
        // Patch this later after we've calculated it with this value
        add_sum: TOC_HEADER_CHKSUM_STAMP,
        serial_num: 0,
        status: 0,
        items_nr: 3,
        valid_len: dtb_end + end_padding,
        version_main: [0; 4],
        version_sub: [0; 2],
        _reserved: [0; 3],
        end: TOC_HEADER_END_MAGIC,
    };

    let mut final_data = Vec::with_capacity((dtb_end + end_padding) as usize);
    final_data.write_all(bytemuck::bytes_of(&header)).unwrap();
    final_data.write_all(bytemuck::bytes_of(&opensbi_entry)).unwrap();
    final_data.write_all(bytemuck::bytes_of(&uboot_entry)).unwrap();
    final_data.write_all(bytemuck::bytes_of(&dtb_entry)).unwrap();

    final_data.resize(2048, 0);
    final_data.write_all(&opensbi_bin).unwrap();
    final_data.resize(final_data.len() + padding as usize, 0);
    final_data.write_all(&dtb).unwrap();
    final_data.resize(final_data.len() + end_padding as usize, 0);

    let checksum = final_data
        .iter()
        .take((dtb_end + end_padding) as usize / 4)
        .copied()
        .fold(0u32, |sum, byte| sum.wrapping_add(byte as u32));

    let header: &mut SbromToc1HeaderInfo =
        &mut bytemuck::cast_slice_mut(&mut final_data[..std::mem::size_of::<SbromToc1HeaderInfo>()])[0];
    header.add_sum = checksum;

    println!("length: {:#X}", dtb_end);
    println!("checksum: {:#X}", checksum);

    img.write_all(&final_data).context("failed to write final data to sdcard image")?;

    Ok(())
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SbromToc1HeaderInfo {
    pub name: [u8; 16],
    pub magic: u32,
    pub add_sum: u32,
    pub serial_num: u32,
    pub status: u32,
    pub items_nr: u32,
    pub valid_len: u32,
    pub version_main: [u8; 4],
    pub version_sub: [u16; 2],
    pub _reserved: [u32; 3],
    pub end: u32,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SbromToc1ItemInfo {
    pub name: [u8; 64],
    pub data_offset: u32,
    pub data_len: u32,
    pub encrypt: u32,
    pub type_: u32,
    pub run_addr: u32,
    pub index: u32,
    pub _reserved: [u32; 69],
    pub end: u32,
}
