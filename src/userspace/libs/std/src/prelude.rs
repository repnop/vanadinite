// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod rust_2021 {
    pub use crate::{dbg, print, println};
    pub use alloc::{
        boxed::Box,
        collections::VecDeque,
        format,
        string::{String, ToString},
        vec,
        vec::Vec,
    };
    pub use core::prelude::rust_2021::*;
    pub use core::{assert, assert_eq, assert_ne, panic, todo, unreachable};
    pub use librust::{task::Tid, taskgroup::TaskGroupShareable};
}
