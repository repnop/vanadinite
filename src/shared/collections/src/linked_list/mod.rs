// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::alloc::Allocator;

/// Doubly-linked list
pub mod doubly;
/// Singly-linked list
pub mod singly;

/// A marker trait that asserts, for any given instance of the `Allocator`, it
/// can be used to to safely `{Singly, Doubly}LinkedList::append(other: &mut
/// {Singly, Doubly}LinkedList<A, T>)`. This is necessary since an
/// implementation of `PartialEq` for any allocator is safe and therefore could
/// always return `true`, even if the allocators point to different memory, thus
/// causing undefined behavior when the linked list nodes would be dropped.
///
/// # Safety
///
/// Only implement this trait for ZST allocators or any allocator which would
/// otherwise reference the same backing memory for every instance of it that is
/// created
pub unsafe trait AllocatorCanMerge: Allocator {}

#[cfg(any(feature = "std", test))]
unsafe impl AllocatorCanMerge for std::alloc::Global {}
