// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

extern crate alloc;
pub use alloc::task::Wake;
pub use core::task::*;

use crate::sync::SyncRefCell;

pub(crate) static INTERRUPT_CALLBACK: SyncRefCell<Option<Box<dyn FnMut(usize)>>> = SyncRefCell::new(None);

pub fn register_interrupt_callback(f: impl FnMut(usize) + 'static) {
    *INTERRUPT_CALLBACK.borrow_mut() = Some(Box::new(f));
}
