// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use present::sync::oneshot::{oneshot, OneshotTx};
use std::{
    collections::BTreeMap,
    sync::{SyncRc, SyncRefCell},
};

type Map<T> = SyncRc<SyncRefCell<BTreeMap<T, VecDeque<OneshotTx<()>>>>>;
pub struct WaitList<T> {
    map: Map<T>,
}

impl<T: Copy + Ord> WaitList<T> {
    pub fn new() -> Self {
        Self { map: SyncRc::new(SyncRefCell::new(BTreeMap::new())) }
    }

    pub async fn acquire(&self, key: T) -> AcquiredToken<T> {
        let mut me = self.map.borrow_mut();

        match me.get_mut(&key) {
            Some(waitlist) => {
                let (tx, rx) = oneshot();
                waitlist.push_back(tx);
                drop(me);

                rx.recv().await;
            }
            None => drop(me.insert(key, VecDeque::new())),
        }

        AcquiredToken(key, SyncRc::clone(&self.map))
    }
}

impl<T> Clone for WaitList<T> {
    fn clone(&self) -> Self {
        Self { map: SyncRc::clone(&self.map) }
    }
}

pub struct AcquiredToken<T: Copy + Ord>(T, Map<T>);

impl<T: Copy + Ord> Drop for AcquiredToken<T> {
    fn drop(&mut self) {
        let mut map = self.1.borrow_mut();
        let waitlist = map.get_mut(&self.0).unwrap();
        match waitlist.len() {
            0 => drop(map.remove(&self.0)),
            _ => {
                let next = waitlist.pop_front().unwrap();
                drop(map);
                next.send(());
            }
        }
    }
}
