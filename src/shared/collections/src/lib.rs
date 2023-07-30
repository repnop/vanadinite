// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

//! A replacement for `alloc` which contains collections that will never panic
//! on allocation failure except when otherwise necessary (e.g. `Clone` impls)

#![feature(allocator_api, array_chunks, const_trait_impl, const_slice_from_raw_parts_mut, slice_ptr_get)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![no_std]

#[cfg(any(test, feature = "std"))]
extern crate std;

/// Hash functions
pub mod hash;
/// An open-addressed with quadratic probing hash table implementation
pub mod hash_map;
/// Linked lists
pub mod linked_list;
/// Least-Recently-Used cache
pub mod lru;
