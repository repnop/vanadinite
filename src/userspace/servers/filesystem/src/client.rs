// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use filesystem::{
    filesystems::{path::Path, FileId, Filesystem, FilesystemError},
    vidl::{
        raw::{File, FileHandle},
        Error, OpenOptions,
    },
};
use std::{collections::BTreeMap, sync::SyncRc};
use vidl::{sync::SharedBuffer, CapabilityPtr};

struct OpenedFile {
    id: FileId,
    filesystem: SyncRc<dyn Filesystem>,
    buffer: SharedBuffer,
}

struct ClientProvider {
    opened_files: BTreeMap<FileHandle, OpenedFile>,
    filesystems: SyncRc<[SyncRc<dyn Filesystem>]>,
}

impl filesystem::vidl::raw::AsyncFilesystemProvider for ClientProvider {
    type Error = ();

    async fn open(&mut self, path: String, options: OpenOptions) -> Result<Result<File, Error>, Self::Error> {
        let fs = &self.filesystems[0];
        let root = fs.root();
        let file = match fs.open(root, Path::new(&path), options.to_file_permissions()).await {
            Ok(file) => file,
            Err(e) => {
                return match e {
                    FilesystemError::DeviceError(_) => Ok(Err(Error::IoError)),
                    FilesystemError::DirectoryNotFound => Ok(Err(Error::FileNotFound)),
                    FilesystemError::FileNotFound => Ok(Err(Error::FileNotFound)),
                    FilesystemError::InvalidPath => Ok(Err(Error::InvalidPath)),
                    FilesystemError::InvalidRoot | FilesystemError::InvalidFileId => Err(()),
                    FilesystemError::OperationNotSupported => Ok(Err(Error::OperationNotSupported)),
                }
            }
        };

        let buffer = SharedBuffer::new(4096).unwrap();
        let buffer2 = unsafe { buffer.clone() };
        let handle = self
            .opened_files
            .last_key_value()
            .map(|(k, _)| FileHandle { id: k.id + 1 })
            .unwrap_or(FileHandle { id: 0 });
        self.opened_files.insert(handle, OpenedFile { id: file, filesystem: SyncRc::clone(fs), buffer });

        Ok(Ok(File { handle, buffer: buffer2 }))
    }

    async fn close(&mut self, handle: FileHandle) -> Result<Result<(), Error>, Self::Error> {
        let opened_file = match self.opened_files.remove(&handle) {
            Some(opened_file) => opened_file,
            None => return Ok(Err(Error::InvalidHandle)),
        };

        match opened_file.filesystem.close(FileId::clone(&opened_file.id)).await {
            Ok(_) => Ok(Ok(())),
            Err(e) => match e {
                FilesystemError::DeviceError(_) => Ok(Err(Error::IoError)),
                FilesystemError::DirectoryNotFound => Ok(Err(Error::FileNotFound)),
                FilesystemError::FileNotFound => Ok(Err(Error::FileNotFound)),
                FilesystemError::InvalidPath => Ok(Err(Error::InvalidPath)),
                FilesystemError::InvalidRoot | FilesystemError::InvalidFileId => Err(()),
                FilesystemError::OperationNotSupported => Ok(Err(Error::OperationNotSupported)),
            },
        }
    }

    async fn read(&mut self, handle: FileHandle) -> Result<Result<usize, Error>, Self::Error> {
        let opened_file = match self.opened_files.get_mut(&handle) {
            Some(opened_file) => opened_file,
            None => return Ok(Err(Error::InvalidHandle)),
        };

        let (len, data) = match opened_file.filesystem.read_file_block(FileId::clone(&opened_file.id)).await {
            Ok(Some(v)) => v,
            Ok(None) => return Ok(Ok(0)),
            Err(e) => {
                return match e {
                    FilesystemError::DeviceError(_) => Ok(Err(Error::IoError)),
                    FilesystemError::DirectoryNotFound => Ok(Err(Error::FileNotFound)),
                    FilesystemError::FileNotFound => Ok(Err(Error::FileNotFound)),
                    FilesystemError::InvalidPath => Ok(Err(Error::InvalidPath)),
                    FilesystemError::InvalidRoot | FilesystemError::InvalidFileId => Err(()),
                    FilesystemError::OperationNotSupported => Ok(Err(Error::OperationNotSupported)),
                }
            }
        };

        opened_file.buffer.copy_from_slice(&data[..len]);

        Ok(Ok(len))
    }
}

pub async fn serve_client(cptr: CapabilityPtr, filesystems: SyncRc<[SyncRc<dyn Filesystem>]>) {
    filesystem::vidl::raw::AsyncFilesystem::new(ClientProvider { opened_files: BTreeMap::new(), filesystems }, cptr)
        .serve()
        .await;
}
