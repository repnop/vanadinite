// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[cfg(feature = "platform.virt")]
pub mod virt;

// FIXME: this is kind of hacky because contexts aren't currently standardized,
// should look for a better way to do it in the future
pub fn current_plic_context() -> usize {
    #[cfg(not(feature = "sifive_u"))]
    return 1 + 2 * crate::HART_ID.get();

    // first context is M-mode E51 monitor core which doesn't support S-mode so
    // we'll always be on hart >=1 which ends up working out to remove the +1
    // from the other fn
    #[cfg(feature = "platform.sifive_u")]
    return 2 * crate::HART_ID.get();
}

pub fn plic_context_for(hart_id: usize) -> usize {
    #[cfg(not(feature = "sifive_u"))]
    return 1 + 2 * hart_id;

    // first context is M-mode E51 monitor core which doesn't support S-mode so
    // we'll always be on hart >=1 which ends up working out to remove the +1
    // from the other fn
    #[cfg(feature = "platform.sifive_u")]
    return 2 * hart_id;
}

pub enum ExitStatus<'a> {
    Ok,
    Error(&'a dyn core::fmt::Display),
}

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
