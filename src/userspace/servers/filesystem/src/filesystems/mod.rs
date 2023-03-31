// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use self::path::Path;
use crate::{
    block_devices::{DataBlock, DeviceError},
    BoxedFuture,
};
use std::sync::SyncRc;

/// BIOS Parameter Block structures
pub mod bpb;
/// FAT32 driver
pub mod fat32;
/// Filesystem path types and helpers
pub mod path;

#[derive(Debug)]
pub enum FilesystemError {
    DeviceError(DeviceError),
    DirectoryNotFound,
    FileNotFound,
    InternalError,
    InvalidFileId,
    InvalidPath,
    InvalidRoot,
    OperationNotSupported,
}

impl From<DeviceError> for FilesystemError {
    fn from(v: DeviceError) -> Self {
        Self::DeviceError(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FileId(u64);

pub struct File {
    id: FileId,
    filesystem: SyncRc<dyn Filesystem>,
}

/// A root directory for a filesystem. This may not correspond to the true root
/// of the filesystem, as new [`Root`]s can be created via
/// [`Filesystem::derive_root`]. Accesses that attempt to leave the root
/// specified by this type will fail.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Root(u64);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FilePermissions(u64);

impl FilePermissions {
    pub const READ: Self = Self(1 << 0);
    pub const OVERWRITE: Self = Self(1 << 1);
    pub const APPEND: Self = Self(1 << 2);
}

impl core::ops::BitOr for FilePermissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for FilePermissions {
    type Output = bool;
    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0) == rhs.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum FileType {
    File,
    Directory,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub struct FileInfo {
    pub filename: String,
    pub file_type: FileType,
}

pub trait Filesystem: Send + Sync {
    /// The true [`Root`] of the filesystem
    fn root(&self) -> Root;
    /// Set the true [`Root`] of the filesystem. To be called once during
    /// initialization to set where the [`Filesystem`] is mounted.
    fn set_root(&mut self, root: &Path);

    /// Derive a new [`Root`] from the given root and a [`Path`]
    fn derive_root(&self, root: Root, path: &Path) -> BoxedFuture<'static, Result<Root, FilesystemError>>;

    /// Create a new [`File`] in the given root with the specified permissions
    fn create(
        &self,
        root: Root,
        path: &Path,
        permissions: FilePermissions,
    ) -> BoxedFuture<'static, Result<FileId, FilesystemError>>;

    fn open(
        &self,
        root: Root,
        path: &Path,
        permissions: FilePermissions,
    ) -> BoxedFuture<'static, Result<FileId, FilesystemError>>;

    fn close(&self, file: FileId) -> BoxedFuture<'static, Result<(), FilesystemError>>;

    fn read_file_block(
        &self,
        file: FileId,
    ) -> BoxedFuture<'static, Result<Option<(usize, DataBlock)>, FilesystemError>>;

    fn exists(&self, root: Root, path: &Path) -> BoxedFuture<'static, Result<Option<FileType>, FilesystemError>>;
    fn list_directory(&self, root: Root, path: &Path) -> BoxedFuture<'static, Result<Vec<FileInfo>, FilesystemError>>;
}
