// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use netstack::{ipv4::IpV4Address, MacAddress};
use present::sync::{mpsc::Sender, oneshot::{OneshotRx, OneshotTx}};
use std::collections::BTreeMap;
use sync::{SpinMutex, SpinRwLock};

pub static ARP_CACHE: ArpCache = ArpCache::new();

pub struct ArpCache {
    cache: SpinMutex<BTreeMap<IpV4Address, MacAddress>>,
    lookup_sender: SpinRwLock<Option<Sender<(IpV4Address, OneshotTx<MacAddress>)>>>,
}

impl ArpCache {
    const fn new() -> Self {
        Self { cache: SpinMutex::new(BTreeMap::new()), lookup_sender: SpinRwLock::new(None) }
    }

    pub fn set_lookup_task_sender(&self, sender: Sender<(IpV4Address, OneshotTx<MacAddress>)>) {
        let mut guard = self.lookup_sender.write();
        if guard.is_none() {
            *guard = Some(sender);
        }
    }

    pub fn lookup(&self, address: IpV4Address) -> Option<MacAddress> {
        self.cache.lock().get(&address).copied()
    }

    pub async fn resolve_and_cache(&'static self, address: IpV4Address) -> MacAddress {
        let (lookup_tx, lookup_rx) = present::sync::oneshot::oneshot();
        self.lookup_sender.read().as_ref().expect("ARP lookup task started").send((address, lookup_tx));

        let mac = lookup_rx.recv().await;
        self.cache.lock().insert(address, mac);
        mac
    }
}
