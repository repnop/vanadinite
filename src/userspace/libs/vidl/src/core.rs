// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub type U8 = u8;
pub type I8 = i8;
pub type U16 = u16;
pub type I16 = i16;
pub type U32 = u32;
pub type I32 = i32;
pub type U64 = u64;
pub type I64 = i64;
pub type U128 = u128;
pub type I128 = i128;

pub type Result<T, E> = core::result::Result<T, E>;
pub type Option<T> = core::option::Option<T>;
pub type String = std::string::String;
pub type Vec<T> = std::vec::Vec<T>;
