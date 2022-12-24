// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(allocator_api, slice_ptr_get)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![no_std]

#[cfg(any(test, feature = "std"))]
extern crate std;

/// Linked lists
pub mod linked_list;
/// Least-Recently-Used cache
pub mod lru;
