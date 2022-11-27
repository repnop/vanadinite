// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(sync_unsafe_cell)]

pub mod executor;
pub mod futures;
pub mod interrupt;
pub mod ipc;
pub mod join;
pub mod sync;
pub mod waker;

pub use executor::{spawn, Present};
pub use present_macros::main;

#[macro_export]
macro_rules! pin {
    ($i:ident) => {
        let mut $i = $i;
        #[allow(unused_mut)]
        let mut $i = unsafe { core::pin::Pin::new_unchecked(&mut $i) };
    };
}
