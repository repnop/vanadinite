// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Debug, Clone, Copy)]
pub struct Bytes(pub usize);

impl From<usize> for Bytes {
    fn from(b: usize) -> Self {
        Self(b)
    }
}
