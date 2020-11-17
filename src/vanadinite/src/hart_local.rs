// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::utils::StaticMut;

const MAX_CPUS: usize = 64;

static HART_LOCAL_INFO: StaticMut<[HartLocalInfo; MAX_CPUS]> = StaticMut::new([HartLocalInfo::new(); MAX_CPUS]);

#[derive(Debug, Default, Clone, Copy)]
pub struct HartLocalInfo {
    pub hart_id: usize,
}

impl HartLocalInfo {
    pub const fn new() -> Self {
        Self { hart_id: usize::max_value() }
    }
}

pub fn hart_local_info() -> HartLocalInfo {
    let tp: usize;
    unsafe { asm!("mv {}, tp", out(reg) tp) };

    unsafe { (&*HART_LOCAL_INFO.get())[tp] }
}

pub fn set_hart_local_info(info: HartLocalInfo) {
    let tp: usize;
    unsafe { asm!("mv {}, tp", out(reg) tp) };

    unsafe { (&mut *HART_LOCAL_INFO.get())[tp] = info };
}

pub unsafe fn init_hart_local_info(hart_id: usize) {
    asm!("mv tp, {}", in(reg) hart_id);
    (&mut *HART_LOCAL_INFO.get())[hart_id] = HartLocalInfo { hart_id };
}
