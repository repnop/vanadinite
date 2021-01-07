use crate::{
    interrupts::assert_interrupts_disabled,
    mem::{paging::VirtualAddress, satp, sfence, virt2phys, SatpMode},
    process::{Process, ProcessState, INIT_PROCESS},
    thread_local,
    trap::TrapFrame,
};
use alloc::collections::VecDeque;
use core::{cell::RefCell, sync::atomic::Ordering};

thread_local! {
    pub static SCHEDULER: RefCell<Scheduler> = RefCell::new(Scheduler::new(Process::load(&elf64::Elf::new(INIT_PROCESS).unwrap())));
}

pub struct Scheduler {
    pub active: Process,
    pub processes: VecDeque<Process>,
}

impl Scheduler {
    pub fn new(init: Process) -> Self {
        Self { active: init, processes: VecDeque::new() }
    }

    pub fn push(this: &RefCell<Self>, process: Process) {
        this.borrow_mut().processes.push_back(process);
    }

    pub fn mark_active_dead(this: &RefCell<Self>) {
        this.borrow_mut().active.state = ProcessState::Dead;
    }

    pub fn active_pid(this: &RefCell<Self>) -> usize {
        this.borrow().active.pid
    }

    pub fn update_active_registers(this: &RefCell<Self>, frame: TrapFrame, pc: usize) {
        let mut this = this.borrow_mut();
        this.active.frame = frame;
        this.active.pc = pc;
    }

    pub fn with_mut_self<T, F: FnOnce(&mut Self) -> T>(this: &RefCell<Self>, f: F) -> T {
        f(&mut *this.borrow_mut())
    }

    pub fn schedule(this: &RefCell<Self>) -> ! {
        assert_interrupts_disabled();
        let (registers, pc) = {
            let mut this = this.borrow_mut();
            let current_dead = this.active.state.is_dead();

            if !this.processes.is_empty() && !current_dead {
                let next = this.processes.pop_front().unwrap();
                let old = core::mem::replace(&mut this.active, next);
                this.processes.push_back(old);
            } else if !this.processes.is_empty() && current_dead {
                log::info!("current process is ded");
                let next = this.processes.pop_front().unwrap();
                this.active = next;
            } else if current_dead {
                unreachable!("we have no process to schedule :(");
            }

            satp(
                SatpMode::Sv39,
                this.active.pid as u16,
                virt2phys(VirtualAddress::from_ptr(this.active.page_table.table())),
            );
            sfence(None, None);

            log::debug!("scheduling process: pid={}, pc={:#p}", this.active.pid, this.active.pc as *mut u8);
            (this.active.frame.registers, this.active.pc)
        };

        let frequency = crate::TIMER_FREQ.load(Ordering::Relaxed);
        let current_time = crate::csr::time::read();
        let target_time = current_time + crate::utils::ticks_per_us(10_000, frequency);
        sbi::timer::set_timer(target_time as u64).unwrap();

        unsafe { return_to_usermode(&registers, pc) }
    }
}

unsafe impl Send for Scheduler {}

#[naked]
#[no_mangle]
unsafe extern "C" fn return_to_usermode(_registers: &crate::trap::Registers, _sepc: usize) -> ! {
    #[rustfmt::skip]
    asm!("
        csrw sepc, a1

        li t0, 1 << 8
        csrc sstatus, t0
        li t0, 1 << 19
        csrs sstatus, t0
        li t0, 1 << 5
        csrs sstatus, t0
        
        ld x1, 0(a0)
        ld x2, 8(a0)
        ld x3, 16(a0)
        ld x4, 24(a0)
        ld x5, 32(a0)
        ld x6, 40(a0)
        ld x7, 48(a0)
        ld x8, 56(a0)
        ld x9, 64(a0)
        ld x11, 80(a0)
        ld x12, 88(a0)
        ld x13, 96(a0)
        ld x14, 104(a0)
        ld x15, 112(a0)
        ld x16, 120(a0)
        ld x17, 128(a0)
        ld x18, 136(a0)
        ld x19, 144(a0)
        ld x20, 152(a0)
        ld x21, 160(a0)
        ld x22, 168(a0)
        ld x23, 176(a0)
        ld x24, 184(a0)
        ld x25, 192(a0)
        ld x26, 200(a0)
        ld x27, 208(a0)
        ld x28, 216(a0)
        ld x29, 224(a0)
        ld x30, 232(a0)
        ld x31, 240(a0)

        ld x10, 72(a0)

        sret
    ", options(noreturn));
}
