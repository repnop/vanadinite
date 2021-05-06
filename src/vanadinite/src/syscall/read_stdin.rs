// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    csr::sstatus::TemporaryUserMemoryAccess,
    io::INPUT_QUEUE,
    mem::paging::{flags, VirtualAddress},
    task::{Task, TaskState},
    trap::TrapFrame,
};

pub fn read_stdin(active_task: &mut Task, virt: VirtualAddress, len: usize, regs: &mut TrapFrame) {
    log::debug!("Attempting to write to memory at {:#p} (len={})", virt, len);

    let valid_memory = {
        let mm = &active_task.memory_manager;

        let flags_start = mm.page_flags(virt);
        let flags_end = mm.page_flags(virt.offset(len));

        flags_start.zip(flags_end).map(|(fs, fe)| fs & flags::WRITE && fe & flags::WRITE).unwrap_or_default()
    };

    if virt.is_kernel_region() {
        log::error!("Process tried to get us to write to our own memory >:(");
        active_task.state = TaskState::Dead;
        return;
    } else if !valid_memory {
        log::error!("Process tried to get us to write to unmapped memory >:(");
        active_task.state = TaskState::Dead;
        return;
    }

    let _guard = TemporaryUserMemoryAccess::new();
    let mut n_written = 0;
    for index in 0..len {
        let value = match INPUT_QUEUE.pop() {
            Some(v) => v,
            None => break,
        };
        unsafe { virt.offset(index).as_mut_ptr().write(value) };
        n_written += 1;
    }

    regs.registers.a0 = n_written;
}
