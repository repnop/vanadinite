// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alloc::{collections::BTreeMap, string::String};
use librust::capabilities::CapabilityPtr;
use sync::SpinRwLock;

#[no_mangle]
static mut ARGS: [usize; 2] = [0; 2];

pub fn args() -> &'static [&'static str] {
    let [argc, argv] = unsafe { ARGS };

    match [argc, argv] {
        [0, _] | [_, 0] => &[],
        [argc, argv] => unsafe { core::slice::from_raw_parts(argv as *const &str, argc) },
    }
}

#[no_mangle]
static mut A2: usize = 0;

// FIXME: how to do this without being gross
pub fn a2() -> usize {
    unsafe { A2 }
}

// FIXME: making this #[thread_local] required a relocation that I don't feel
// like implementing right now
pub(crate) static CAP_MAP: SpinRwLock<BTreeMap<String, CapabilityPtr>> = SpinRwLock::new(BTreeMap::new());

pub fn lookup_capability(service: &str) -> Option<CapabilityPtr> {
    CAP_MAP.read().get(service).copied()
}
