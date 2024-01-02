// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(
    adt_const_params,
    allocator_api,
    const_option,
    const_trait_impl,
    exposed_provenance,
    generic_const_exprs,
    inline_const_pat,
    layout_for_ptr,
    never_type,
    ptr_metadata,
    slice_ptr_get,
    slice_ptr_len,
    strict_provenance,
    try_trait_v2
)]
#![no_std]
#![allow(incomplete_features)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod capabilities;
pub mod error;
pub mod mem;
pub mod syscalls;
pub mod task;
pub mod units;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<T, U> Either<T, U> {
    #[track_caller]
    pub fn unwrap_left(self) -> T {
        match self {
            Either::Left(t) => t,
            Either::Right(_) => panic!("attempted to unwrap the opposing variant of an `Either`"),
        }
    }

    #[track_caller]
    pub fn unwrap_right(self) -> U {
        match self {
            Either::Right(u) => u,
            Either::Left(_) => panic!("attempted to unwrap the opposing variant of an `Either`"),
        }
    }

    #[track_caller]
    pub fn left_or(self, default: T) -> T {
        match self {
            Either::Left(t) => t,
            Either::Right(_) => default,
        }
    }

    #[track_caller]
    pub fn right_or(self, default: U) -> U {
        match self {
            Either::Right(u) => u,
            Either::Left(_) => default,
        }
    }
}
