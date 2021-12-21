// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod drivers;

use librust::mem::DmaRegion;

fn main() {
    let addr = if true { todo!() } else { unsafe { &*(core::mem::align_of::<()>() as *const _) } };
    let mut drv = drivers::virtio::BlockDevice::new(addr).unwrap();

    let mem: DmaRegion<[u8; 512]> = unsafe { DmaRegion::zeroed().unwrap().assume_init() };
    drv.queue_read(0, mem.physical_address());
}
