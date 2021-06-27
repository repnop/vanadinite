// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[cfg(feature = "platform.virt")]
#[path = "virt.rs"]
mod impl_;

#[cfg(feature = "platform.sifive_u")]
#[path = "sifive_u.rs"]
mod impl_;

#[cfg(feature = "platform.nezha")]
#[path = "nezha.rs"]
mod impl_;

pub use impl_::*;

pub enum ExitStatus<'a> {
    Ok,
    Error(&'a dyn core::fmt::Display),
}

#[cfg(feature = "platform.virt")]
pub fn exit(status: ExitStatus) -> ! {
    impl_::exit(match status {
        ExitStatus::Ok => impl_::ExitStatus::Pass,
        ExitStatus::Error(_) => impl_::ExitStatus::Fail(1),
    })
}

#[cfg(not(feature = "platform.virt"))]
pub fn exit(status: ExitStatus) -> ! {
    use sbi::{
        probe_extension,
        system_reset::{system_reset, ResetReason, ResetType, EXTENSION_ID},
        ExtensionAvailability,
    };

    match probe_extension(EXTENSION_ID) {
        ExtensionAvailability::Available(_) => system_reset(
            ResetType::Shutdown,
            match status {
                ExitStatus::Ok => ResetReason::NoReason,
                ExitStatus::Error(_) => ResetReason::SystemFailure,
            },
        )
        .unwrap(),
        ExtensionAvailability::Unavailable => {
            crate::csr::sstatus::disable_interrupts();
            loop {
                unsafe { asm!("nop") };
            }
        }
    }
}
