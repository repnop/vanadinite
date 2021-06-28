// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::ecall;

/// Base extension ID
pub const EXTENSION_ID: usize = 0x10;

/// SBI specification version implemented by the SBI implementation
#[derive(Debug, Clone, Copy)]
pub struct SbiSpecVersion {
    /// Major version number
    pub major: usize,
    /// Minor version number
    pub minor: usize,
}

/// Retrieve the SBI specification version
pub fn spec_version() -> SbiSpecVersion {
    let value = unsafe { ecall(Default::default(), EXTENSION_ID, 0).unwrap() };
    SbiSpecVersion { major: (value >> 24) & 0x7f, minor: value & 0xff_ffff }
}

/// SBI implementation name
#[derive(Debug, Clone, Copy)]
pub enum SbiImplId {
    #[allow(missing_docs)]
    BerkeleyBootLoader,
    #[allow(missing_docs)]
    OpenSbi,
    #[allow(missing_docs)]
    Xvisor,
    #[allow(missing_docs)]
    Kvm,
    #[allow(missing_docs)]
    RustSbi,
    #[allow(missing_docs)]
    Diosix,
    #[allow(missing_docs)]
    Other(usize),
}

impl SbiImplId {
    /// Convert to the `usize` implementation ID value
    pub fn into_usize(self) -> usize {
        match self {
            SbiImplId::BerkeleyBootLoader => 0,
            SbiImplId::OpenSbi => 1,
            SbiImplId::Xvisor => 2,
            SbiImplId::Kvm => 3,
            SbiImplId::RustSbi => 4,
            SbiImplId::Diosix => 5,
            SbiImplId::Other(n) => n,
        }
    }

    fn from_usize(n: usize) -> Self {
        match n {
            0 => SbiImplId::BerkeleyBootLoader,
            1 => SbiImplId::OpenSbi,
            2 => SbiImplId::Xvisor,
            3 => SbiImplId::Kvm,
            4 => SbiImplId::RustSbi,
            5 => SbiImplId::Diosix,
            n => SbiImplId::Other(n),
        }
    }
}

/// Retrieve the SBI implementation ID
pub fn impl_id() -> SbiImplId {
    let value = unsafe { ecall(Default::default(), EXTENSION_ID, 1).unwrap() };
    SbiImplId::from_usize(value)
}

/// Retrieve the SBI implementation's version
pub fn impl_version() -> usize {
    unsafe { ecall(Default::default(), EXTENSION_ID, 2).unwrap() }
}

/// Extension availability information returned by `probe_extension`
#[derive(Debug, Clone, Copy)]
pub enum ExtensionAvailability {
    /// The extension is available, along with its extension-specific non-zero
    /// value
    Available(core::num::NonZeroUsize),
    /// The extension is unavailable
    Unavailable,
}

impl ExtensionAvailability {
    /// Helper method for converting `ExtensionAvailability` to a bool
    pub fn is_available(self) -> bool {
        match self {
            ExtensionAvailability::Available(_) => true,
            ExtensionAvailability::Unavailable => false,
        }
    }
}

/// Probe the availability of the extension ID `id`
pub fn probe_extension(id: usize) -> ExtensionAvailability {
    let value = unsafe { ecall([id, 0, 0, 0, 0, 0], EXTENSION_ID, 3).unwrap() };

    match value {
        0 => ExtensionAvailability::Unavailable,
        n => ExtensionAvailability::Available(unsafe { core::num::NonZeroUsize::new_unchecked(n) }),
    }
}

/// Retrieve the value of `mvendorid` CSR
pub fn mvendorid() -> usize {
    unsafe { ecall(Default::default(), EXTENSION_ID, 4).unwrap() }
}

/// Retrieve the value of the `marchid` CSR
pub fn marchid() -> usize {
    unsafe { ecall(Default::default(), EXTENSION_ID, 5).unwrap() }
}

/// Retrieve the value of the `mimpid` CSR
pub fn mimpid() -> usize {
    unsafe { ecall(Default::default(), EXTENSION_ID, 6).unwrap() }
}
