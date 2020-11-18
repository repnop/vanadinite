// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

const MAX_CPUS: usize = 64;

static HART_LOCAL_INFO: [HartLocalInfo; MAX_CPUS] = [HartLocalInfo::new(); MAX_CPUS];

#[derive(Debug)]
pub struct HartLocalInfo {
    hart_id: AtomicUsize,
    asid: AtomicU16,
}

impl HartLocalInfo {
    pub const fn new() -> Self {
        Self { hart_id: AtomicUsize::new(usize::max_value()), asid: AtomicU16::new(u16::max_value()) }
    }

    pub fn hart_id(&self) -> usize {
        self.hart_id.load(Ordering::Relaxed)
    }

    pub fn asid(&self) -> u16 {
        self.asid.load(Ordering::Relaxed)
    }

    pub fn set_asid(&self, asid: u16) {
        self.asid.store(asid, Ordering::Relaxed)
    }
}

pub fn hart_local_info() -> &'static HartLocalInfo {
    let tp: usize;
    unsafe { asm!("mv {}, tp", out(reg) tp) };

    &HART_LOCAL_INFO[tp]
}

pub fn init_hart_local_info(hart_id: usize) {
    unsafe { asm!("mv tp, {}", in(reg) hart_id) };
    HART_LOCAL_INFO[hart_id].hart_id.store(hart_id, Ordering::Relaxed)
}
