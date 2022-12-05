// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

//!

#![feature(async_fn_in_trait, inline_const)]
#![allow(incomplete_features)]
#![warn(missing_docs)]

macro_rules! assert_struct_size {
    ($t:ty, $n:literal) => {
        const _: () = match core::mem::size_of::<$t>() {
            $n => {}
            _ => panic!(concat!(
                "Struct ",
                stringify!($t),
                "'s size does not match the expected size of ",
                stringify!($n),
                " bytes"
            )),
        };
    };
}

/// Helper type for specifying a heap-allocated [`core::future::Future`]
pub type BoxedFuture<'a, T> = core::pin::Pin<Box<dyn core::future::Future<Output = T> + Send + Sync + 'a>>;

/// Traits and types relevant to block devices & device drivers
pub mod block_devices;
/// Filesystem drivers
pub mod filesystems;
/// Partitioning discovery
pub mod partitions;
pub mod probe;

/// VIDL interface
pub mod vidl {
    pub use raw::{Error, OpenOptions};
    use vidl::CapabilityPtr;

    pub mod raw {
        use crate::filesystems::FilePermissions;

        vidl::vidl_include!("filesystem");

        impl OpenOptions {
            pub fn to_file_permissions(self) -> FilePermissions {
                match self {
                    OpenOptions::Append => FilePermissions::APPEND,
                    OpenOptions::Overwrite => FilePermissions::OVERWRITE,
                    OpenOptions::ReadOnly => FilePermissions::READ,
                }
            }
        }
    }

    #[derive(Default)]
    struct BufferCursor {
        len: usize,
        consumed: usize,
    }

    impl BufferCursor {
        fn remaining(&self) -> usize {
            self.len - self.consumed
        }

        fn range(&self) -> core::ops::Range<usize> {
            self.consumed..self.len
        }

        fn consume(&mut self, amount: usize) -> core::ops::Range<usize> {
            assert!(self.consumed + amount <= self.len);
            let current = self.consumed;
            self.consumed += amount;

            current..current + amount
        }
    }

    pub struct File {
        client: raw::FilesystemClient,
        file: raw::File,
        cursor: BufferCursor,
    }

    impl File {
        pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
            if buffer.is_empty() {
                return Ok(0);
            }

            let remaining = self.cursor.remaining();
            let read = if buffer.len() <= remaining {
                let buf_len = buffer.len();
                buffer[..remaining].copy_from_slice(&self.file.buffer.read()[self.cursor.consume(buf_len)]);
                buffer.len()
            } else {
                if remaining != 0 {
                    buffer[..remaining].copy_from_slice(&self.file.buffer.read()[self.cursor.range()]);
                }

                let mut client_offset = remaining;
                while buffer.len() > client_offset {
                    let len @ 1.. = self.client.read(self.file.handle)? else { break };
                    let amount_to_copy = usize::min(buffer.len() - client_offset, len);
                    let client_range = client_offset..client_offset + amount_to_copy;

                    self.cursor = BufferCursor { len, consumed: 0 };

                    buffer[client_range].copy_from_slice(&self.file.buffer.read()[self.cursor.consume(amount_to_copy)]);
                    client_offset += amount_to_copy;
                }

                client_offset
            };

            Ok(read)
        }

        pub fn close(self) -> Result<(), Error> {
            // Safety: we `core::mem::forget(self)` so only the cloned copy will
            // get dropped
            let _buffer = unsafe { self.file.buffer.clone() };

            self.client.close(self.file.handle)?;
            core::mem::forget(self);

            Ok(())
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            // If there's an error closing a file, don't panic
            let _ = self.client.close(self.file.handle);
        }
    }

    pub struct FilesystemClient {
        client: raw::FilesystemClient,
        cptr: CapabilityPtr,
    }

    impl FilesystemClient {
        pub fn new(cptr: CapabilityPtr) -> Self {
            Self { client: raw::FilesystemClient::new(cptr), cptr }
        }

        pub fn open<'a>(&'a self, path: &str, options: OpenOptions) -> Result<File, Error> {
            let file = self.client.open(path, options)?;
            Ok(File { client: raw::FilesystemClient::new(self.cptr), file, cursor: BufferCursor::default() })
        }
    }
}
