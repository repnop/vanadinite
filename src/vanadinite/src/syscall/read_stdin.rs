use crate::{
    csr::sstatus::TemporaryUserMemoryAccess,
    io::INPUT_QUEUE,
    mem::paging::VirtualAddress,
    scheduler::{Scheduler, SCHEDULER},
    trap::TrapFrame,
};

pub fn read_stdin(virt: VirtualAddress, len: usize, regs: &mut TrapFrame) {
    log::debug!("Attempting to print memory at {:#p} (len={})", virt, len);
    let valid_memory = Scheduler::with_mut_self(&*SCHEDULER, |s| {
        s.active.page_table.is_valid_readable(virt) && s.active.page_table.is_valid_readable(virt.offset(len))
    });

    if virt.is_kernel_region() {
        log::error!("Process tried to get us to write to our own memory >:(");
        Scheduler::mark_active_dead(&*SCHEDULER);
        return;
    } else if !valid_memory {
        log::error!("Process tried to get us to write to unmapped memory >:(");
        Scheduler::mark_active_dead(&*SCHEDULER);
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
