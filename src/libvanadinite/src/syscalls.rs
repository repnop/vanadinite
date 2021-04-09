// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[derive(Debug)]
#[repr(C)]
pub enum SyscallNumbers {
    Exit = 0,
    Print = 1,
    ReadStdin = 2,
}

pub mod print {
    #[derive(Debug)]
    #[repr(C)]
    pub enum PrintErr {
        NoAccess,
    }
}
