// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::GLOBAL_EXECUTOR;
use std::sync::Arc;

pub type ArcWaker = Arc<Waker>;

pub struct Waker {
    pub(crate) task_id: u64,
}

impl std::task::Wake for Waker {
    fn wake(self: std::sync::Arc<Self>) {
        GLOBAL_EXECUTOR.borrow_mut().awaken(self.task_id);
    }
}
