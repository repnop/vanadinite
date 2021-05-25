// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod rust_2018 {
    pub use crate::{print, println};
    pub use alloc::collections::VecDeque;
    pub use alloc::prelude::v1::*;
    pub use core::prelude::v1::*;
}
