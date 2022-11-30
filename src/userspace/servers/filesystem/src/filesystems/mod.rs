// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use self::path::Path;
use crate::BoxedFuture;
use std::sync::SyncRc;

/// FAT32 driver
pub mod fat32;
/// Filesystem path types and helpers
pub mod path;

pub enum FilesystemError {
    DirectoryNotFound,
    FileNotFound,
    InvalidPath,
    OperationNotSupported,
}

pub struct File {
    id: u64,
    root: Root,
    filesystem: SyncRc<dyn Filesystem>,
}

/// A root directory for a filesystem. This may not correspond to the true root
/// of the filesystem, as new [`Root`]s can be created via
/// [`Filesystem::derive_root`]. Accesses that attempt to leave the root
/// specified by this type will fail.
#[derive(Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Root(u64);

pub trait Filesystem {
    /// The true [`Root`] of the filesystem
    fn root(&self) -> Root;

    /// Derive a new [`Root`] from the given root and a [`Path`]
    fn derive_root(&self, root: Root, path: &Path) -> BoxedFuture<'static, Result<Root, FilesystemError>>;
}
