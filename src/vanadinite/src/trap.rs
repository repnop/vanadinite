// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    interrupts::{isr::isr_entry, PLIC},
    mem::paging::VirtualAddress,
    scheduler::Scheduler,
    syscall,
};

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Registers {
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
}

impl Registers {
    pub fn sp(&self) -> *mut u8 {
        self.sp as *mut u8
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct FloatingPointRegisters {
    pub f0: usize,
    pub f1: usize,
    pub f2: usize,
    pub f3: usize,
    pub f4: usize,
    pub f5: usize,
    pub f6: usize,
    pub f7: usize,
    pub f8: usize,
    pub f9: usize,
    pub f10: usize,
    pub f11: usize,
    pub f12: usize,
    pub f13: usize,
    pub f14: usize,
    pub f15: usize,
    pub f16: usize,
    pub f17: usize,
    pub f18: usize,
    pub f19: usize,
    pub f20: usize,
    pub f21: usize,
    pub f22: usize,
    pub f23: usize,
    pub f24: usize,
    pub f25: usize,
    pub f26: usize,
    pub f27: usize,
    pub f28: usize,
    pub f29: usize,
    pub f30: usize,
    pub f31: usize,
    pub fscr: usize,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct TrapFrame {
    pub registers: Registers,
    pub fp_registers: FloatingPointRegisters,
}

const INTERRUPT_BIT: usize = 1 << 63;

#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub enum Trap {
    // Software interrupts
    #[allow(clippy::identity_op)]
    UserSoftwareInterrupt = INTERRUPT_BIT | 0,
    SupervisorSoftwareInterrupt = INTERRUPT_BIT | 1,
    MachineSoftwareInterrupt = INTERRUPT_BIT | 3,

    // Timers
    UserTimerInterrupt = INTERRUPT_BIT | 4,
    SupervisorTimerInterrupt = INTERRUPT_BIT | 5,
    MachineTimerInterrupt = INTERRUPT_BIT | 7,

    // External interrupts
    UserExternalInterrupt = INTERRUPT_BIT | 8,
    SupervisorExternalInterrupt = INTERRUPT_BIT | 9,
    MachineExternalInterrupt = INTERRUPT_BIT | 11,

    // General faults/ecalls
    InstructionAddressMisaligned = 0,
    InstructionAccessFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadAddressMisaligned = 4,
    LoadAccessFault = 5,
    StoreAddressMisaligned = 6,
    StoreAccessFault = 7,
    UserModeEnvironmentCall = 8,
    SupervisorModeEnvironmentCall = 9,
    MachineModeEnvironmentCall = 11,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StorePageFault = 15,

    Reserved = usize::max_value(),
}

impl Trap {
    pub fn from_cause(cause: usize) -> Self {
        use Trap::*;

        match cause {
            0x8000000000000000 => UserSoftwareInterrupt,
            0x8000000000000001 => SupervisorSoftwareInterrupt,
            0x8000000000000003 => MachineSoftwareInterrupt,

            0x8000000000000004 => UserTimerInterrupt,
            0x8000000000000005 => SupervisorTimerInterrupt,
            0x8000000000000007 => MachineTimerInterrupt,

            0x8000000000000008 => UserExternalInterrupt,
            0x8000000000000009 => SupervisorExternalInterrupt,
            0x800000000000000B => MachineExternalInterrupt,

            0 => InstructionAddressMisaligned,
            1 => InstructionAccessFault,
            2 => IllegalInstruction,
            3 => Breakpoint,
            4 => LoadAddressMisaligned,
            5 => LoadAccessFault,
            6 => StoreAddressMisaligned,
            7 => StoreAccessFault,
            8 => UserModeEnvironmentCall,
            9 => SupervisorModeEnvironmentCall,
            11 => MachineModeEnvironmentCall,
            12 => InstructionPageFault,
            13 => LoadPageFault,
            15 => StorePageFault,

            _ => Reserved,
        }
    }
}

#[no_mangle]
pub extern "C" fn trap_handler(regs: &mut TrapFrame, sepc: usize, scause: usize, stval: usize) -> usize {
    log::debug!("we trappin' on hart {}: {:x?}", crate::HART_ID.get(), regs);
    log::debug!("TCB: {:?}", unsafe { &*crate::process::THREAD_CONTROL_BLOCK.get() });
    log::debug!("scause: {:?}, sepc: {:#x}, stval (as ptr): {:#p}", Trap::from_cause(scause), sepc, stval as *mut u8);

    let trap_kind = Trap::from_cause(scause);
    match trap_kind {
        Trap::LoadPageFault | Trap::StorePageFault => {
            let sepc = VirtualAddress::new(sepc);
            let stval = VirtualAddress::new(stval);

            match sepc.is_kernel_region() {
                true => panic!("kernel {:?} at address {:#p}", trap_kind, stval),
                false => {
                    let pid = Scheduler::active_pid();
                    log::error!("Active process (pid: {}) {:?} at address {:#p}, killing", pid, trap_kind, stval);
                    Scheduler::mark_active_dead();
                }
            }
        }
        Trap::SupervisorTimerInterrupt => {
            Scheduler::update_active_registers(*regs, sepc);
            Scheduler::schedule();
        }
        Trap::UserModeEnvironmentCall => {
            match regs.registers.a0 {
                0 => syscall::exit::exit(),
                1 => syscall::print::print(
                    VirtualAddress::new(regs.registers.a1),
                    regs.registers.a2,
                    VirtualAddress::new(regs.registers.a3),
                ),
                2 => syscall::read_stdin::read_stdin(VirtualAddress::new(regs.registers.a1), regs.registers.a2, regs),
                n => {
                    log::error!("Unknown syscall number: {}", n);
                    Scheduler::mark_active_dead();
                }
            }

            Scheduler::update_active_registers(*regs, sepc + 4);
            Scheduler::schedule();
        }
        Trap::SupervisorExternalInterrupt => {
            let plic = PLIC.lock();
            if let Some(claimed) = plic.claim(crate::platform::current_plic_context()) {
                if let Some((callback, private)) = isr_entry(claimed.interrupt_id()) {
                    callback(claimed.interrupt_id(), private).unwrap();
                }

                claimed.complete();
            }
        }
        trap => panic!("Ignoring trap: {:?}, sepc: {:#x}, stval: {:#x}", trap, sepc, stval),
    }

    sepc
}

#[naked]
#[no_mangle]
unsafe extern "C" fn stvec_trap_shim() -> ! {
    #[rustfmt::skip]
    asm!("
        .align 4
        # Disable interrupts
        csrci sstatus, 2
        csrrw s0, sscratch, s0

        sd sp, 16(s0)
        sd tp, 24(s0)

        ld sp, 0(s0)
        ld tp, 8(s0)

        addi sp, sp, -512

        sd x1, 0(sp)

        # push original sp
        ld x1, 16(s0)
        sd x1, 8(sp)

        sd x3, 16(sp)

        # store original tp
        ld x1, 24(s0)
        sd x1, 24(sp)

        sd x5, 32(sp)
        sd x6, 40(sp)
        sd x7, 48(sp)
        
        # store original s0
        csrr x1, sscratch
        sd x1, 56(sp)

        # restore x1's value
        ld x1, 0(sp)

        # now we can restore sscratch to its original
        csrw sscratch, s0

        sd x9, 64(sp)
        sd x10, 72(sp)
        sd x11, 80(sp)
        sd x12, 88(sp)
        sd x13, 96(sp)
        sd x14, 104(sp)
        sd x15, 112(sp)
        sd x16, 120(sp)
        sd x17, 128(sp)
        sd x18, 136(sp)
        sd x19, 144(sp)
        sd x20, 152(sp)
        sd x21, 160(sp)
        sd x22, 168(sp)
        sd x23, 176(sp)
        sd x24, 184(sp)
        sd x25, 192(sp)
        sd x26, 200(sp)
        sd x27, 208(sp)
        sd x28, 216(sp)
        sd x29, 224(sp)
        sd x30, 232(sp)
        sd x31, 240(sp)
        fsd f0, 248(sp)
        fsd f1, 256(sp)
        fsd f2, 264(sp)
        fsd f3, 272(sp)
        fsd f4, 280(sp)
        fsd f5, 288(sp)
        fsd f6, 296(sp)
        fsd f7, 304(sp)
        fsd f8, 312(sp)
        fsd f9, 320(sp)
        fsd f10, 328(sp)
        fsd f11, 336(sp)
        fsd f12, 344(sp)
        fsd f13, 352(sp)
        fsd f14, 360(sp)
        fsd f15, 368(sp)
        fsd f16, 376(sp)
        fsd f17, 384(sp)
        fsd f18, 392(sp)
        fsd f19, 400(sp)
        fsd f20, 408(sp)
        fsd f21, 416(sp)
        fsd f22, 424(sp)
        fsd f23, 432(sp)
        fsd f24, 440(sp)
        fsd f25, 448(sp)
        fsd f26, 456(sp)
        fsd f27, 464(sp)
        fsd f28, 472(sp)
        fsd f29, 480(sp)
        fsd f30, 488(sp)
        fsd f31, 496(sp)

        frcsr t0
        sd t0, 504(sp)

        mv a0, sp

        csrr a1, sepc
        csrr a2, scause
        csrr a3, stval

        li s0, 1 << 5
        # Reenable interrupts after sret (set SPIE)
        csrs sstatus, s0

        call trap_handler

        csrw sepc, a0

        ld x1, 0(sp)
        # skip x2 as its the stack pointer
        ld x3, 16(sp)
        ld x4, 24(sp)
        ld x5, 32(sp)
        ld x6, 40(sp)
        ld x7, 48(sp)
        ld x8, 56(sp)
        ld x9, 64(sp)
        ld x10, 72(sp)
        ld x11, 80(sp)
        ld x12, 88(sp)
        ld x13, 96(sp)
        ld x14, 104(sp)
        ld x15, 112(sp)
        ld x16, 120(sp)
        ld x17, 128(sp)
        ld x18, 136(sp)
        ld x19, 144(sp)
        ld x20, 152(sp)
        ld x21, 160(sp)
        ld x22, 168(sp)
        ld x23, 176(sp)
        ld x24, 184(sp)
        ld x25, 192(sp)
        ld x26, 200(sp)
        ld x27, 208(sp)
        ld x28, 216(sp)
        ld x29, 224(sp)
        ld x30, 232(sp)
        ld x31, 240(sp)

        sc.d zero, zero, 0(sp)
        csrr sp, sscratch
        ld sp, 16(sp)

        # gtfo
        sret
    ", options(noreturn));
}
