// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2023 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

/// Hasher builder for [`FxHasher`]
pub type FxBuildHasher = core::hash::BuildHasherDefault<FxHasher>;

const K: u64 = 0x517cc1b727220a95;

/// A hasher using the FX hash algorithm
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FxHasher(u64);

impl FxHasher {
    /// Create a new [`FxHasher`]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Hash the given `u64` value
    #[must_use]
    pub const fn hash(self, value: u64) -> Self {
        Self((self.0.rotate_left(5) ^ value).wrapping_mul(K))
    }

    /// Return the generated hash value after inputs have been processed
    pub const fn finish(self) -> u64 {
        self.0
    }
}

impl core::hash::BuildHasher for FxHasher {
    type Hasher = Self;

    fn build_hasher(&self) -> Self::Hasher {
        Self::new()
    }
}

impl core::hash::Hasher for FxHasher {
    fn write(&mut self, mut bytes: &[u8]) {
        for chunk in bytes.array_chunks() {
            *self = self.hash(u64::from_ne_bytes(*chunk));
            bytes = &bytes[8..];
        }

        if bytes.len() >= 4 {
            *self = self.hash(u64::from(u32::from_ne_bytes(<[u8; 4]>::try_from(&bytes[..4]).unwrap())));
            bytes = &bytes[4..];
        }

        for byte in bytes {
            *self = self.hash(u64::from(*byte));
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
