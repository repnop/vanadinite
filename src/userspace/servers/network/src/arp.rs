// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use netstack::{ipv4::IpV4Address, MacAddress};
use present::sync::{mpsc::Sender, oneshot::OneshotTx};
use std::{collections::BTreeMap, sync::SyncRefCell};

pub static ARP_CACHE: ArpCache = ArpCache::new();

pub struct ArpCache {
    cache: SyncRefCell<BTreeMap<IpV4Address, MacAddress>>,
    lookup_sender: SyncRefCell<Option<Sender<(IpV4Address, OneshotTx<MacAddress>)>>>,
}

impl ArpCache {
    const fn new() -> Self {
        Self { cache: SyncRefCell::new(BTreeMap::new()), lookup_sender: SyncRefCell::new(None) }
    }

    pub fn set_lookup_task_sender(&self, sender: Sender<(IpV4Address, OneshotTx<MacAddress>)>) {
        let mut guard = self.lookup_sender.borrow_mut();
        if guard.is_none() {
            *guard = Some(sender);
        }
    }

    pub fn lookup(&self, address: IpV4Address) -> Option<MacAddress> {
        self.cache.borrow().get(&address).copied()
    }

    pub async fn resolve_and_cache(&'static self, address: IpV4Address) -> MacAddress {
        let (lookup_tx, lookup_rx) = present::sync::oneshot::oneshot();
        self.lookup_sender.borrow().as_ref().expect("ARP lookup task started").send((address, lookup_tx));

        let mac = lookup_rx.recv().await;
        self.cache.borrow_mut().insert(address, mac);
        mac
    }
}
