// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alchemy::PackedStruct;
use endian::{LittleEndianU16, LittleEndianU32};

use super::{
    bpb::BiosParameterBlock,
    path::{Path, PathBuf},
    FileId, FilePermissions, FileType, Filesystem, FilesystemError, Root,
};
use crate::{
    block_devices::{BlockDevice, DataBlock, DeviceError, SectorIndex},
    waitlist::WaitList,
    BoxedFuture,
};
use core::{
    cell::{Ref, RefMut},
    future::Future,
    ops::ControlFlow,
};
use std::{
    collections::BTreeMap,
    sync::{SyncRc, SyncRefCell},
};

/// Filesystem driver for the FAT32 filesystem
pub struct Fat32 {
    inner: SyncRc<SyncRefCell<Fat32Inner>>,
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(C)]
struct DirectoryData {
    filename: [u8; 11],
    attributes: DirectoryAttributes,
    _reserved1: [u8; 8],
    start_cluster_high: LittleEndianU16,
    _reserved2: [u8; 4],
    start_cluster_low: LittleEndianU16,
    file_size: LittleEndianU32,
}

impl DirectoryData {
    fn start_cluster(&self) -> Cluster {
        Cluster((u64::from(self.start_cluster_high.to_ne()) << 16) | u64::from(self.start_cluster_low.to_ne()))
    }

    fn entry_kind(&self) -> DirectoryEntryKind {
        match self.filename[0] {
            0x00 => DirectoryEntryKind::EndOfEntries,
            0xE5 => DirectoryEntryKind::Deleted,
            _ => DirectoryEntryKind::Present,
        }
    }

    fn long_filename_chars(&self) -> Option<impl Iterator<Item = char>> {
        if self.attributes & DirectoryAttributes::LONG_FILENAME {
            // Very Long Filename entries use UCS-2...
            let entry_bytes = self.as_bytes();
            let filename_characters = [
                char::from_u32(u32::from(entry_bytes[0x01])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x03])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x05])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x07])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x09])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x0E])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x10])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x12])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x14])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x16])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x18])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x1C])).unwrap_or('\u{0}'),
                char::from_u32(u32::from(entry_bytes[0x1E])).unwrap_or('\u{0}'),
            ];

            return Some(filename_characters.into_iter().filter(|c| !matches!(c, '\u{0}' | '\u{FF}')));
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectoryEntryKind {
    Deleted,
    EndOfEntries,
    Present,
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
struct DirectoryAttributes(u8);

#[allow(dead_code)]
impl DirectoryAttributes {
    /// File or directory is read-only
    const READ_ONLY: Self = Self(1 << 0);
    /// File or directory is hidden
    const HIDDEN: Self = Self(1 << 1);
    /// File or directory is a system component
    const SYSTEM: Self = Self(1 << 2);
    const VOLUME_ID: Self = Self(1 << 3);
    /// This is a long filename extension entry
    const LONG_FILENAME: Self = Self(0x0F);
    /// This is a directory
    const SUBDIRECTORY: Self = Self(1 << 4);
    /// Not used
    const ARCHIVE: Self = Self(1 << 5);
}

impl core::ops::BitOr for DirectoryAttributes {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for DirectoryAttributes {
    type Output = bool;
    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct Cluster(u64);

impl Cluster {
    const END_OF_CLUSTER_CHAIN: Self = Self(u64::MAX);
    fn to_sector(self, first_cluster_sector: SectorIndex, sectors_per_cluster: u64) -> SectorIndex {
        first_cluster_sector + ((self.0 - 2) * sectors_per_cluster)
    }
}

#[derive(Debug, Clone, Copy, PackedStruct)]
#[repr(transparent)]
struct FatEntry(LittleEndianU32);

impl FatEntry {
    const fn kind(self) -> FatEntryKind {
        match self.0.to_ne() & 0x0FFFFFFF {
            0x00000000 => FatEntryKind::Unused,
            0x0FFFFFF8..=0x0FFFFFFF => FatEntryKind::LastClusterOfFile,
            n => FatEntryKind::Cluster(Cluster(n as u64)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FatEntryKind {
    Cluster(Cluster),
    LastClusterOfFile,
    Unused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenFileInfo {
    total_size: u64,
    total_read: u64,
    current_cluster: Cluster,
}

struct Fat32Inner {
    block_device: SyncRc<dyn BlockDevice>,
    // FIXME: ensure these boundaries are never crossed
    #[allow(dead_code)]
    first_sector: SectorIndex,
    #[allow(dead_code)]
    last_sector: SectorIndex,
    fat_start: SectorIndex,
    clusters_start: SectorIndex,
    sectors_per_cluster: u64,
    root_directory_first_cluster: Cluster,
    roots: BTreeMap<Root, PathBuf>,
    open_files: BTreeMap<FileId, OpenFileInfo>,
    sensitive_sector_waitlist: WaitList<SectorIndex>,
}

impl Fat32 {
    #[allow(missing_docs)]
    pub fn new(
        block_device: SyncRc<dyn BlockDevice>,
        bpb: &BiosParameterBlock,
        first_sector: SectorIndex,
        last_sector: SectorIndex,
    ) -> Self {
        let reserved_sectors = u64::from(bpb.reserved_sector_count.get());
        let total_fat_size = u64::from(bpb.num_fats) * u64::from(bpb.fat32_fat_size.get());
        Self {
            inner: SyncRc::new(SyncRefCell::new(Fat32Inner {
                block_device,
                first_sector,
                last_sector,
                fat_start: first_sector + reserved_sectors,
                clusters_start: first_sector + reserved_sectors + total_fat_size,
                sectors_per_cluster: u64::from(bpb.sectors_per_cluster),
                root_directory_first_cluster: Cluster(u64::from(bpb.root_cluster.get())),
                roots: BTreeMap::new(),
                open_files: BTreeMap::new(),
                sensitive_sector_waitlist: WaitList::new(),
            })),
        }
    }

    fn cloned(&self) -> Self {
        Self { inner: SyncRc::clone(&self.inner) }
    }

    fn inner(&self) -> Ref<'_, Fat32Inner> {
        self.inner.borrow()
    }

    fn inner_mut(&self) -> RefMut<'_, Fat32Inner> {
        self.inner.borrow_mut()
    }
}

impl Filesystem for Fat32 {
    fn root(&self) -> Root {
        Root(0)
    }

    fn set_root(&mut self, root: &Path) {
        assert!(
            self.inner_mut().roots.insert(Root(0), PathBuf::from(root)).is_none(),
            "`set_root` called multiple times!"
        );
    }

    fn derive_root(&self, root: Root, path: &Path) -> BoxedFuture<'static, Result<Root, FilesystemError>> {
        let path = PathBuf::from(path);
        let me = self.cloned();
        Box::pin(async move {
            match me.exists(root, path.as_ref()).await? {
                Some(FileType::Directory) => {
                    let mut me = me.inner_mut();
                    let last_root_id =
                        me.roots.last_key_value().map(|(r, _)| r.clone()).expect("`set_root` never called!");
                    let new_root = Root(last_root_id.0 + 1);
                    me.roots.insert(new_root, path);

                    Ok(Root(last_root_id.0 + 1))
                }
                Some(FileType::File) => Err(FilesystemError::InvalidPath),
                None => Err(FilesystemError::DirectoryNotFound),
            }
        })
    }

    fn create(
        &self,
        root: Root,
        path: &Path,
        permissions: FilePermissions,
    ) -> BoxedFuture<'static, Result<FileId, FilesystemError>> {
        todo!()
    }

    fn open(
        &self,
        root: Root,
        path: &Path,
        permissions: FilePermissions,
    ) -> BoxedFuture<'static, Result<FileId, FilesystemError>> {
        let path = match self.inner().roots.get(&root) {
            Some(root_path) => root_path.join(path),
            None => return Box::pin(core::future::ready(Err(FilesystemError::InvalidRoot))),
        };

        let this = self.cloned();
        let me = this.inner();
        let root_directory_cluster = me.root_directory_first_cluster;
        let first_cluster_sector = me.clusters_start;
        let sectors_per_cluster = me.sectors_per_cluster;
        let device = SyncRc::clone(&me.block_device);
        drop(me);

        Box::pin(async move {
            let maybe_found =
                find_path(&*device, first_cluster_sector, root_directory_cluster, sectors_per_cluster, &path).await?;

            let (cluster_start, file_size) = match maybe_found {
                Some(directory_data) => match directory_data.attributes & DirectoryAttributes::SUBDIRECTORY {
                    true => return Err(FilesystemError::FileNotFound),
                    false => (directory_data.start_cluster(), u64::from(directory_data.file_size.to_ne())),
                },
                None => return Err(FilesystemError::FileNotFound),
            };

            let mut me = this.inner_mut();
            let next_file_id = me.open_files.last_key_value().map(|(k, _)| FileId(k.0 + 1)).unwrap_or(FileId(0));
            me.open_files.insert(
                next_file_id.clone(),
                OpenFileInfo { total_size: file_size, total_read: 0, current_cluster: cluster_start },
            );
            drop(me);

            Ok(next_file_id)
        })
    }

    fn close(&self, file: FileId) -> BoxedFuture<'static, Result<(), FilesystemError>> {
        Box::pin(core::future::ready(match self.inner_mut().open_files.remove(&file) {
            Some(_) => Ok(()),
            None => Err(FilesystemError::InvalidFileId),
        }))
    }

    fn read_file_block(
        &self,
        file: FileId,
    ) -> BoxedFuture<'static, Result<Option<(usize, DataBlock)>, FilesystemError>> {
        let this = self.cloned();
        let me = this.inner();
        let mut open_file_info = match me.open_files.get(&file) {
            Some(OpenFileInfo { current_cluster: Cluster::END_OF_CLUSTER_CHAIN, .. }) => {
                return Box::pin(core::future::ready(Ok(None)))
            }
            Some(OpenFileInfo { total_size, total_read, .. }) if total_read == total_size => {
                return Box::pin(core::future::ready(Ok(None)))
            }
            Some(info) => *info,
            None => return Box::pin(core::future::ready(Err(FilesystemError::InvalidFileId))),
        };
        let fat_start = me.fat_start;
        let first_cluster_sector = me.clusters_start;
        let sectors_per_cluster = me.sectors_per_cluster;
        let device = SyncRc::clone(&me.block_device);
        drop(me);

        Box::pin(async move {
            let cluster_byte_size = sectors_per_cluster * /* FIXME: don't assume sector byte size */ 512;
            if open_file_info.total_read % cluster_byte_size == 0 && open_file_info.total_read != 0 {
                // println!("\n\n\n\n\n{cluster_byte_size} - {open_file_info:?}\n\n\n\n\n\n");
                let fat_sector = fat_start + (open_file_info.current_cluster.0 * 4) / 512;
                let data = device.read(fat_sector).await?;
                let fat_entry =
                    FatEntry::try_slice_from_bytes(&data).unwrap()[(open_file_info.current_cluster.0 % 128) as usize];

                match fat_entry.kind() {
                    FatEntryKind::Cluster(next_cluster) => open_file_info.current_cluster = next_cluster,
                    FatEntryKind::LastClusterOfFile => {
                        this.inner_mut().open_files.get_mut(&file).unwrap().current_cluster =
                            Cluster::END_OF_CLUSTER_CHAIN;
                        return Ok(None);
                    }
                    FatEntryKind::Unused => {
                        println!(
                            "[filesystem] Detected bad FAT entry for cluster {}!",
                            open_file_info.current_cluster.0
                        );
                        return Err(FilesystemError::InternalError);
                    }
                }
            }

            let sector_offset = match sectors_per_cluster > 1 {
                true => open_file_info.total_read / /* FIXME: don't assume sector byte size */ 512,
                false => 0,
            };

            let next_data_sector =
                open_file_info.current_cluster.to_sector(first_cluster_sector, sectors_per_cluster) + sector_offset;

            let data = device.read(next_data_sector).await?;

            let amount_read = if open_file_info.total_read + 512 > open_file_info.total_size {
                open_file_info.total_size - open_file_info.total_read
            } else {
                /* FIXME: don't assume sector byte size */
                512
            };

            open_file_info.total_read += amount_read;
            *this.inner_mut().open_files.get_mut(&file).unwrap() = open_file_info;

            Ok(Some((amount_read as usize, data)))
        })
    }

    fn exists(&self, root: Root, path: &Path) -> BoxedFuture<'static, Result<Option<FileType>, FilesystemError>> {
        let path = match self.inner().roots.get(&root) {
            Some(root_path) => root_path.join(path),
            None => return Box::pin(core::future::ready(Err(FilesystemError::InvalidRoot))),
        };

        let this = self.cloned();
        let me = this.inner();
        let root_directory_cluster = me.root_directory_first_cluster;
        let first_cluster_sector = me.clusters_start;
        let sectors_per_cluster = me.sectors_per_cluster;
        let device = SyncRc::clone(&me.block_device);
        drop(me);

        Box::pin(async move {
            let maybe_found =
                find_path(&*device, first_cluster_sector, root_directory_cluster, sectors_per_cluster, &path).await?;

            Ok(match maybe_found {
                Some(directory_data) => match directory_data.attributes & DirectoryAttributes::SUBDIRECTORY {
                    true => Some(FileType::Directory),
                    false => Some(FileType::File),
                },
                None => None,
            })
        })
    }
}

async fn find_path(
    device: &dyn BlockDevice,
    first_cluster_sector: SectorIndex,
    root_directory_cluster: Cluster,
    sectors_per_cluster: u64,
    path: &Path,
) -> Result<Option<DirectoryData>, FilesystemError> {
    let Some(filename) = path.file_name() else { return Err(FilesystemError::InvalidPath) };
    let Some(parent) = path.parent() else { return Err(FilesystemError::InvalidPath) };
    let mut directories = parent.compontents();
    // Skip root directory name since we already have the root directory
    // cluster #
    let _ = directories.next();

    let mut cluster_start = root_directory_cluster;
    for component in directories {
        // Cluster numbering starts at 2
        let directory_cluster = cluster_start.to_sector(first_cluster_sector, sectors_per_cluster);
        let matching_entry =
            with_directory_entries(device, directory_cluster, sectors_per_cluster, |vlfn, directory_data| async move {
                // We hit a file, skip over it
                if !(directory_data.attributes & DirectoryAttributes::SUBDIRECTORY) {
                    return ControlFlow::Continue(());
                }

                match vlfn {
                    Some(name) if name == component => {
                        return ControlFlow::Break(directory_data.start_cluster());
                    }
                    _ => {
                        if &directory_data.filename[..] == component.as_bytes() {
                            return ControlFlow::Break(directory_data.start_cluster());
                        }
                    }
                }

                ControlFlow::Continue(())
            });

        match matching_entry.await? {
            Some(cluster) => cluster_start = cluster,
            None => return Ok(None),
        }
    }

    // Loop over the directory the file should be contained within
    let directory_cluster = cluster_start.to_sector(first_cluster_sector, sectors_per_cluster);
    let matching_file =
        with_directory_entries(device, directory_cluster, sectors_per_cluster, |vlfn, directory_data| async move {
            match vlfn {
                Some(name) if name == filename => {
                    return ControlFlow::Break(directory_data);
                }
                _ => {
                    if &directory_data.filename[..] == filename.as_bytes() {
                        return ControlFlow::Break(directory_data);
                    }
                }
            }

            ControlFlow::Continue(())
        });

    Ok(matching_file.await?)
}

async fn with_directory_entries<T, F, Fut>(
    device: &dyn BlockDevice,
    directory_start: SectorIndex,
    sectors_per_cluster: u64,
    f: F,
) -> Result<Option<T>, DeviceError>
where
    T: 'static,
    F: Fn(Option<String>, DirectoryData) -> Fut,
    Fut: Future<Output = ControlFlow<T>>,
{
    let mut vlfn = String::new();
    // Iterate over each sector in the cluster of directory data
    for i in 0..sectors_per_cluster {
        let cluster_data_block = device.read(directory_start + i).await?;
        let directory_entries = DirectoryData::try_slice_from_bytes(&cluster_data_block).unwrap();

        for directory_data in directory_entries {
            // Insert long filename extension data if needbe
            if let Some(chars) = directory_data.long_filename_chars() {
                // FIXME: find a better way to do this
                for (i, c) in chars.enumerate() {
                    vlfn.insert(i, c);
                }

                continue;
            }

            match directory_data.entry_kind() {
                // We've hit the end of the directory entries
                DirectoryEntryKind::EndOfEntries => return Ok(None),
                // Skip deleted entries
                DirectoryEntryKind::Deleted => {
                    vlfn.clear();
                    continue;
                }
                DirectoryEntryKind::Present => {}
            }

            let name = match vlfn.is_empty() {
                true => None,
                false => Some(vlfn.clone()),
            };

            if let ControlFlow::Break(val) = f(name, *directory_data).await {
                return Ok(Some(val));
            }

            vlfn.clear();
        }
    }

    Ok(None)
}
