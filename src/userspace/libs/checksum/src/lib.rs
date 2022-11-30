// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

// TODO: replace the CRC32 implementation in `netstack` with this one when
// there's a trait for endianness
/// CRC32 checksum calculation
pub mod crc32;
