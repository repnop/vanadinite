// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2026 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![allow(incomplete_features)]
#![feature(
    allocator_api,
    coerce_unsized,
    dropck_eyepatch,
    generic_const_exprs,
    layout_for_ptr,
    never_type,
    pin_coerce_unsized_trait,
    ptr_as_uninit,
    ptr_metadata,
    unsize
)]

#[cfg(test)]
extern crate std;

pub mod boxed;
pub mod store;
pub mod try_clone;
