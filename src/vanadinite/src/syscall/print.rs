use crate::{
    csr::sstatus::TemporaryUserMemoryAccess, io::ConsoleDevice, mem::paging::VirtualAddress, scheduler::Scheduler,
};
use libvanadinite::{syscalls::print::PrintErr, KResult};

pub fn print(virt: VirtualAddress, len: usize, res_out: VirtualAddress) {
    log::debug!("Attempting to print memory at {:#p} (len={})", virt, len);
    let (valid_memory, valid_res) = Scheduler::with_mut_self(|s| {
        let active = s.processes.front_mut().unwrap();

        (
            active.page_table.is_valid_readable(virt) && active.page_table.is_valid_readable(virt.offset(len)),
            active.page_table.is_valid_writable(res_out),
        )
    });

    if !valid_res {
        Scheduler::mark_active_dead();
        return;
    }

    let _guard = TemporaryUserMemoryAccess::new();
    let res_out: *mut KResult<(), PrintErr> = res_out.as_mut_ptr().cast();

    if virt.is_kernel_region() {
        log::error!("Process tried to get us to read from our own memory >:(");
        unsafe { *res_out = KResult::Err(PrintErr::NoAccess) };
        return;
    } else if !valid_memory {
        log::error!("Process tried to get us to read from unmapped memory >:(");
        unsafe { *res_out = KResult::Err(PrintErr::NoAccess) };
        return;
    }

    let mut console = crate::io::CONSOLE.lock();
    let bytes = unsafe { core::slice::from_raw_parts(virt.as_ptr(), len) };
    for byte in bytes {
        console.write(*byte);
    }

    unsafe { *res_out = KResult::Ok(()) };
}
