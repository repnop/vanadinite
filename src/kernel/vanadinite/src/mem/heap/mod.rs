// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod free_list;

use free_list::FreeListAllocator;

#[cfg(any(not(any(feature = "vmalloc.allocator.buddy")), feature = "vmalloc.allocator.freelist"))]
#[global_allocator]
pub static HEAP_ALLOCATOR: FreeListAllocator = FreeListAllocator::new();
