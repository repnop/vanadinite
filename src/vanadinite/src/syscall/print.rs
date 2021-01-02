use crate::{
    arch::csr::sstatus::TemporaryUserMemoryAccess,
    io::ConsoleDevice,
    mem::paging::VirtualAddress,
    scheduler::{Scheduler, SCHEDULER},
};

pub fn print(virt: VirtualAddress, len: usize) {
    log::debug!("Attempting to print memory at {:#p} (len={})", virt, len);
    let valid_memory = Scheduler::with_mut_self(&*SCHEDULER, |s| {
        s.active.page_table.is_valid_readable(virt) && s.active.page_table.is_valid_readable(virt.offset(len))
    });

    if virt.is_kernel_region() {
        log::error!("Process tried to get us to read from our own memory >:(");
        Scheduler::mark_active_dead(&*SCHEDULER);
        return;
    } else if !valid_memory {
        log::error!("Process tried to get us to read from unmapped memory >:(");
        Scheduler::mark_active_dead(&*SCHEDULER);
        return;
    }

    let _guard = TemporaryUserMemoryAccess::new();
    let mut console = crate::io::CONSOLE.lock();
    let bytes = unsafe { core::slice::from_raw_parts(virt.as_ptr(), len) };
    for byte in bytes {
        console.write(*byte);
    }
}
