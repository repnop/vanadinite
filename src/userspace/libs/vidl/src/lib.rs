// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(slice_ptr_get)]

#[macro_export]
macro_rules! vidl_include {
    ($vidl:literal) => {
        include!(concat!(env!("OUT_DIR"), concat!("/", $vidl, ".rs")));
    };
}

#[doc(hidden)]
pub use librust::{
    capabilities::{Capability, CapabilityDescription, CapabilityPtr, CapabilityRights, CapabilityWithDescription},
    mem::MemoryAllocation,
    syscalls::channel::{ChannelMessage, ChannelReadFlags},
    units::Bytes,
};

pub mod core;
pub mod sync;
pub mod materialize {
    pub use materialize::*;
}

#[doc(hidden)]
pub mod internal {
    pub use librust::syscalls::{
        channel::{read_kernel_message, ChannelReadFlags, KernelMessage},
        mem::MemoryPermissions,
    };
    pub use std::ipc::IpcChannel;
}
