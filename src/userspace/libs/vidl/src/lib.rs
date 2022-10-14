// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[macro_export]
macro_rules! vidl_include {
    ($vidl:literal) => {
        include!(concat!(env!("OUT_DIR"), concat!("/", $vidl, ".rs")));
    };
}

pub mod core;
pub mod materialize {
    pub use materialize::*;
}
