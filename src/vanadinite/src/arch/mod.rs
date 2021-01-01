// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod csr;
#[cfg(feature = "virt")]
pub mod virt;

pub enum ExitStatus<'a> {
    Ok,
    Error(&'a dyn core::fmt::Display),
}

#[cfg(feature = "virt")]
pub fn exit(status: ExitStatus) -> ! {
    let exit_status = match status {
        ExitStatus::Ok => virt::ExitStatus::Pass,
        ExitStatus::Error(msg) => virt::ExitStatus::Fail(1),
    };

    virt::exit(exit_status)
}

#[cfg(not(feature = "virt"))]
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
            crate::arch::csr::sstatus::disable_interrupts();
            loop {
                unsafe { asm!("nop") };
            }
        }
    }
}
