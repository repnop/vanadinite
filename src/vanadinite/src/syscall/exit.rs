// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::scheduler::Scheduler;

pub fn exit() {
    log::info!("Killing active process (pid: {})", Scheduler::active_pid());
    Scheduler::mark_active_dead();
}
