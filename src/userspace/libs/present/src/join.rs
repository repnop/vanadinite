// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::sync::oneshot::OneshotRx;

pub struct JoinHandle<T: Send + 'static> {
    oneshot: OneshotRx<T>,
}

impl<T: Send + 'static> JoinHandle<T> {
    pub(crate) fn new(oneshot: OneshotRx<T>) -> Self {
        Self { oneshot }
    }

    pub async fn join(self) -> T {
        self.oneshot.recv().await
    }
}
