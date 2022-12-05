// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::block_devices::SectorIndex;

/// GUID Partition Table parsing
pub mod gpt;
/// Master Boot Record parsing
pub mod mbr;

/// The partition number on a disk
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartitionId(u32);

impl PartitionId {
    /// Create a new [`PartitionId`]
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the numeric value for the [`PartitionId`]
    pub fn get(self) -> u32 {
        self.0
    }
}
