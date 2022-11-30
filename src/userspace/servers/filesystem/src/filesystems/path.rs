// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[repr(transparent)]
pub struct Path(str);

impl Path {
    pub fn new(path: &str) -> &Self {
        unsafe { &*(path as *const str as *const Self) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct PathBuf(String);

impl PathBuf {
    pub fn new() -> Self {
        Self(String::new())
    }
}
