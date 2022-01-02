// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub(crate) struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut stdio = match crate::env::lookup_capability("stdio") {
            Some(stdio) => crate::ipc::IpcChannel::new(stdio),
            None => return Ok(()),
        };
        let _ = stdio.send_bytes(s, &[]);
        // let _ = librust::syscalls::print(s.as_bytes());
        Ok(())
    }
}
