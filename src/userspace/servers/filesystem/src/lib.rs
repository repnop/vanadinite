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
pub type BoxedFuture<'a, T> = core::pin::Pin<Box<dyn core::future::Future<Output = T> + 'a>>;

/// Traits and types relevant to block devices & device drivers
pub mod block_devices;
/// Filesystem drivers
pub mod filesystems;
/// Partitioning discovery
pub mod partitions;

/// VIDL interface
pub mod vidl {
    vidl::vidl_include!("filesystem");
}
