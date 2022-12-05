// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;

use crate::{
    block_devices::{BlockDevice, DeviceError, SectorIndex},
    filesystems::{bpb::BiosParameterBlock, fat32::Fat32, Filesystem},
    partitions::{
        gpt::{GptHeader, GptPartitionEntry, Guid},
        mbr::{MasterBootRecord, PartitionType},
    },
};
use std::{rc::Rc, sync::SyncRc};

/// Probe the given block device for any filesystem partitions, returning a
/// `Vec` of known filesystems
pub async fn filesystem_probe(device: SyncRc<dyn BlockDevice>) -> Result<Vec<SyncRc<dyn Filesystem>>, DeviceError> {
    println!("Probing for filesystems!");
    let mbr_data = device.read(SectorIndex::new(0)).await?;
    let mbr = MasterBootRecord::try_from_byte_slice(&mbr_data).unwrap();

    if mbr.partitions[0].parition_type != PartitionType::GPT_PROTECTIVE_MBR {
        todo!("support non-GPT partitioned disks?");
    }

    let gpt_data = device.read(SectorIndex::new(1)).await?;
    let gpt = GptHeader::try_from_byte_slice(&gpt_data).unwrap();
    let mut filesystems = Vec::new();

    let partition_entry_size = gpt.partition_table_info.entry_size.to_ne();
    let block_size = device.block_size().get() as u32;

    assert_eq!(
        block_size % partition_entry_size,
        0,
        "TODO: GPT Partition entry size is not a even division of the device's block size"
    );

    let total_blocks = (gpt.partition_table_info.entry_count.to_ne() * partition_entry_size) / block_size;
    let starting_sector = SectorIndex::new(gpt.partition_table_info.entry_start_lba.to_ne());
    for i in 0..total_blocks {
        let data = device.read(starting_sector + i as u64).await?;

        let entries = GptPartitionEntry::try_slice_from_bytes(&data).unwrap();
        for entry in entries {
            if entry.type_guid != Guid::UNUSED {
                println!(
                    "[{}] Inspecting partition: {}",
                    entry.unique_guid,
                    entry
                        .name
                        .as_str()
                        .map(|s| match s.len() {
                            0 => "<unnamed>",
                            _ => s,
                        })
                        .unwrap_or("<partition name not UTF-8>")
                );
            }

            match entry.type_guid {
                Guid::UNUSED => continue,
                Guid::MICROSOFT_BASIC_DATA => {
                    println!("[{}] Type: Microsoft Basic Data -- probing for BPB", entry.unique_guid);
                    let maybe_bpb_data = device.read(SectorIndex::new(entry.start_lba.to_ne())).await?;
                    let maybe_bpb = BiosParameterBlock::try_from_byte_slice(&maybe_bpb_data).unwrap();

                    if !maybe_bpb.signature_word.verify() {
                        println!("[{}] BPB signature invalid -- skipping...", entry.unique_guid);
                        continue;
                    }

                    match (maybe_bpb.fat16_total_sectors.get(), maybe_bpb.fat32_total_sectors.get()) {
                        (0, 1..) => {}
                        (_, 0) => {
                            println!("[{}] FAT12/16 detected -- unsupported, skipping...", entry.unique_guid);
                            continue;
                        }
                        (_, _) => {
                            println!("[{}] Invalid BPB configuration -- skipping...", entry.unique_guid);
                            continue;
                        }
                    }

                    if maybe_bpb.bytes_per_sector.get() == 0 {
                        println!("[{}] exFAT detected -- unsupported, skipping...", entry.unique_guid);
                        continue;
                    }

                    let root_dir_sectors = ((maybe_bpb.fat16_root_entry_count.get() as u32 * 32)
                        + (maybe_bpb.bytes_per_sector.get() as u32 - 1))
                        / maybe_bpb.bytes_per_sector.get() as u32;
                    let fat_size = match maybe_bpb.fat16_fat_size.get() {
                        0 => maybe_bpb.fat32_fat_size.get(),
                        n => u32::from(n),
                    };

                    let sector_count = match maybe_bpb.fat16_total_sectors.get() {
                        0 => maybe_bpb.fat32_total_sectors.get(),
                        n => u32::from(n),
                    };

                    let n_data_sectors = sector_count
                        - (maybe_bpb.reserved_sector_count.get() as u32
                            + (maybe_bpb.num_fats as u32 * fat_size)
                            + root_dir_sectors);

                    match n_data_sectors {
                        1..=4084 => {
                            println!("[{}] FAT12 detected -- unsupported, skipping...", entry.unique_guid);
                            continue;
                        }
                        4085..=65524 => {
                            println!("[{}] FAT16 detected -- unsupported, skipping...", entry.unique_guid);
                            continue;
                        }
                        _ => println!("[{}] FAT32 confirmed!", entry.unique_guid),
                    }

                    println!("[{}] FAT32 detected -- creating filesystem driver", entry.unique_guid);

                    let mut fat32 = Fat32::new(
                        SyncRc::clone(&device),
                        maybe_bpb,
                        SectorIndex::new(entry.start_lba.to_ne()),
                        SectorIndex::new(entry.end_lba.to_ne()),
                    );

                    // FIXME: this shouldn't be here, probaqbly need to make it
                    // a `&self` method instead...
                    fat32.set_root(crate::filesystems::path::Path::new("/"));
                    filesystems.push(SyncRc::from_rc(Rc::new(fat32) as Rc<dyn Filesystem>));
                }
                Guid::LINUX_FILESYSTEM_DATA => println!("[{}] TODO: check for ext2/3/4", entry.unique_guid),
                ty => println!("[{}] Skipping unknown type GUID: {}", entry.unique_guid, ty),
            }
        }
    }

    Ok(filesystems)
}
