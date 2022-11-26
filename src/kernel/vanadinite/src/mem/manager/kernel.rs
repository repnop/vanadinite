// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

// use super::{address_map::Kernelspace, AddressMap};
// use crate::mem::paging::PageTable;
// use core::mem::MaybeUninit;

// pub struct KernelMemoryManager {
//     root_page_table: MaybeUninit<PageTable>,
//     address_map: AddressMap<Kernelspace>,
// }

// impl KernelMemoryManager {
//     pub const fn empty() -> Self {
//         Self { root_page_table: core::ptr::null_mut(), address_map: AddressMap::empty() }
//     }

//     pub unsafe fn init(&mut self, root_page_table: PageTable) {
//         self.root_page_table.write(root_page_table);
//         self.address_map.init_empty();
//     }
// }
