// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::Ordering;

use crate::{
    csr::{self},
    interrupts::{isr::invoke_isr, PLIC},
    mem::{
        manager::AddressRegion,
        paging::{flags::Flags, VirtualAddress},
        region::MemoryRegion,
    },
    scheduler::{CURRENT_TASK, SCHEDULER},
    syscall,
    task::TaskState,
    utils::ticks_per_us,
};

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct GeneralRegisters {
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

impl GeneralRegisters {
    pub fn sp(&self) -> *mut u8 {
        self.sp as *mut u8
    }
}

impl core::fmt::Debug for GeneralRegisters {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        struct Hex<T: core::fmt::LowerHex>(T);
        impl<T: core::fmt::LowerHex> core::fmt::Debug for Hex<T> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{:#x}", self.0)
            }
        }

        f.debug_struct("GeneralRegisters")
            .field("ra", &Hex(self.ra))
            .field("sp", &Hex(self.sp))
            .field("gp", &Hex(self.gp))
            .field("tp", &Hex(self.tp))
            .field("t0", &Hex(self.t0))
            .field("t1", &Hex(self.t1))
            .field("t2", &Hex(self.t2))
            .field("s0", &Hex(self.s0))
            .field("s1", &Hex(self.s1))
            .field("a0", &Hex(self.a0))
            .field("a1", &Hex(self.a1))
            .field("a2", &Hex(self.a2))
            .field("a3", &Hex(self.a3))
            .field("a4", &Hex(self.a4))
            .field("a5", &Hex(self.a5))
            .field("a6", &Hex(self.a6))
            .field("a7", &Hex(self.a7))
            .field("s2", &Hex(self.s2))
            .field("s3", &Hex(self.s3))
            .field("s4", &Hex(self.s4))
            .field("s5", &Hex(self.s5))
            .field("s6", &Hex(self.s6))
            .field("s7", &Hex(self.s7))
            .field("s8", &Hex(self.s8))
            .field("s9", &Hex(self.s9))
            .field("s10", &Hex(self.s10))
            .field("s11", &Hex(self.s11))
            .field("t3", &Hex(self.t3))
            .field("t4", &Hex(self.t4))
            .field("t5", &Hex(self.t5))
            .field("t6", &Hex(self.t6))
            .finish()
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
    pub sepc: usize,
    pub registers: GeneralRegisters,
}

impl core::ops::Deref for TrapFrame {
    type Target = GeneralRegisters;

    fn deref(&self) -> &Self::Target {
        &self.registers
    }
}

impl core::ops::DerefMut for TrapFrame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.registers
    }
}

const INTERRUPT_BIT: usize = 1 << 63;

#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

    Reserved = usize::MAX,
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
pub extern "C" fn trap_handler(regs: &mut TrapFrame, scause: usize, stval: usize) {
    log::trace!("we trappin' on hart {}: {:x?}", crate::HART_ID.get(), regs);
    if Trap::from_cause(scause) != Trap::UserModeEnvironmentCall
        || regs.a0 != (librust::syscalls::Syscall::DebugPrint as usize)
    {
        log::debug!(
            "scause: {:?}, sepc: {:#x}, stval (as ptr): {:#p} (syscall? {:?})",
            Trap::from_cause(scause),
            regs.sepc,
            stval as *mut u8,
            (Trap::from_cause(scause) == Trap::UserModeEnvironmentCall)
                .then(|| librust::syscalls::Syscall::from_usize(regs.a0))
                .flatten()
        );
    }

    let trap_kind = Trap::from_cause(scause);
    match trap_kind {
        Trap::SupervisorTimerInterrupt => SCHEDULER.schedule(TaskState::Ready),
        Trap::UserModeEnvironmentCall => match syscall::handle(regs) {
            syscall::Outcome::Completed => regs.sepc += 4,
            syscall::Outcome::Blocked => SCHEDULER.schedule(TaskState::Blocked),
        },
        Trap::SupervisorExternalInterrupt => {
            // FIXME: there has to be a better way
            if let Some(plic) = &*PLIC.lock() {
                if let Some(claimed) = plic.claim(crate::platform::current_plic_context()) {
                    log::debug!("External interrupt for: {:?}", claimed);

                    let interrupt_id = claimed.interrupt_id();
                    match invoke_isr(plic, claimed, interrupt_id) {
                        Ok(_) => log::trace!("ISR (interrupt ID: {}) completed successfully", interrupt_id),
                        Err(e) => log::error!("Error during ISR: {}", e),
                    }
                }
            }
        }
        Trap::LoadPageFault | Trap::StorePageFault | Trap::InstructionPageFault => {
            let sepc = VirtualAddress::new(regs.sepc);
            let stval = VirtualAddress::new(stval);
            match sepc.is_kernel_region() {
                // We should always have marked memory regions up front from the initial mapping
                true => {
                    let active = CURRENT_TASK.get();

                    match active.mutable_state.try_lock() {
                        Some(active) => log::error!(
                            "Process memory map during error:\n{:#?}",
                            active.memory_manager.address_map_debug(Some(stval))
                        ),
                        None => log::error!("Deadlock would have occurred for process map printing"),
                    }
                    panic!("[KERNEL BUG] {:?} @ pc={:#p}: stval={:#p} regs={:x?}", trap_kind, sepc, stval, regs);
                }
                false => {
                    let active_task_lock = CURRENT_TASK.get();
                    let mut active_task = active_task_lock.mutable_state.lock();
                    let memory_manager = &mut active_task.memory_manager;

                    //log::info!("{:#?}", memory_manager.region_for(stval));

                    let valid = match memory_manager.region_for(stval) {
                        None | Some(AddressRegion { region: None, .. }) => false,
                        Some(AddressRegion { region: Some(MemoryRegion::GuardPage), .. }) => {
                            log::error!("Process hit a guard page, stack overflow?");
                            false
                        }
                        _ => match trap_kind {
                            Trap::LoadPageFault | Trap::InstructionPageFault => {
                                match memory_manager.page_flags(stval) {
                                    Some(flags) => {
                                        (flags & Flags::READ)
                                            && memory_manager.modify_page_flags(stval, |f| f | Flags::ACCESSED)
                                    }
                                    None => false,
                                }
                            }
                            Trap::StorePageFault => match memory_manager.page_flags(stval) {
                                Some(flags) => {
                                    (flags & Flags::WRITE)
                                        && memory_manager
                                            .modify_page_flags(stval, |f| f | Flags::DIRTY | Flags::ACCESSED)
                                }
                                None => false,
                            },
                            _ => unreachable!(),
                        },
                    };

                    match valid {
                        true => crate::mem::sfence(Some(stval), None),
                        false => {
                            log::error!(
                                "Process {} died to a {:?} @ {:#p} (PC: {:#p})",
                                active_task_lock.name,
                                trap_kind,
                                stval,
                                sepc,
                            );
                            log::error!("Register dump:\n{:?}", regs);
                            // log::error!("Stack dump (last 32 values):\n");
                            // let mut sp = regs.registers.sp as *const u64;
                            // for _ in 0..32 {
                            //     log::error!("{:#p}: {:#x}", sp, unsafe { *sp });
                            //     sp = unsafe { sp.offset(1) };
                            // }
                            log::error!(
                                "Memory map:\n{:#?}",
                                active_task.memory_manager.address_map_debug(Some(stval))
                            );
                            log::error!("Phys addr (if any): {:?}", active_task.memory_manager.resolve(stval));

                            drop(active_task);
                            drop(active_task_lock);

                            SCHEDULER.schedule(TaskState::Dead)
                        }
                    }
                }
            }
        }
        trap => panic!("Ignoring trap: {:?}, sepc: {:#x}, stval: {:#x}", trap, regs.sepc, stval),
    }

    if log::log_enabled!(log::Level::Debug) {
        let task = CURRENT_TASK.get();
        log::debug!("Scheduling {:?}, pc: {:#x}", task.name, regs.sepc);
    }

    sbi::timer::set_timer(csr::time::read() + ticks_per_us(10_000, crate::TIMER_FREQ.load(Ordering::Relaxed))).unwrap();
}

/// # Safety
/// nice try
#[naked]
#[no_mangle]
#[repr(align(4))]
pub unsafe extern "C" fn stvec_trap_shim() -> ! {
    #[rustfmt::skip]
    core::arch::asm!(r#"
        // Interrupts are disabled when we enter a trap
        // Switch `t6` and `sscratch`
        csrrw t6, sscratch, t6

        // Store current stack pointer temporarily
        sd sp, 24(t6)

        // Load kernel's stack pointer
        ld sp, 0(t6)
        addi sp, sp, {TRAP_FRAME_SIZE}

        // ###############################################
        // # Begin storing userspace state in trap frame #
        // ###############################################
        sd ra, 8(sp)

        // Load and save the userspace stack pointer using
        // the now freed `ra` register
        ld ra, 24(t6)
        sd ra, 16(sp)

        // Save the other registers regularly
        sd gp, 24(sp)
        sd tp, 32(sp)
        sd t0, 40(sp)
        sd t1, 48(sp)
        sd t2, 56(sp)
        sd s0, 64(sp)
        sd s1, 72(sp)
        sd a0, 80(sp)
        sd a1, 88(sp)
        sd a2, 96(sp)
        sd a3, 104(sp)
        sd a4, 112(sp)
        sd a5, 120(sp)
        sd a6, 128(sp)
        sd a7, 136(sp)
        sd s2, 144(sp)
        sd s3, 152(sp)
        sd s4, 160(sp)
        sd s5, 168(sp)
        sd s6, 176(sp)
        sd s7, 184(sp)
        sd s8, 192(sp)
        sd s9, 200(sp)
        sd s10, 208(sp)
        sd s11, 216(sp)
        sd t3, 224(sp)
        sd t4, 232(sp)
        sd t5, 240(sp)

        ld tp, 8(t6)
        ld gp, 16(t6)

        // Swap `t6` and `sscratch` again
        csrrw t6, sscratch, t6
        sd t6, 248(sp)

        // Save `sepc`
        csrr t6, sepc
        sd t6, 0(sp)

        mv a0, sp
        csrr a1, scause
        csrr a2, stval

        // Check if floating point registers are dirty
        csrr s0, sstatus
        srli s0, s0, 13
        andi s0, s0, 3
        li s1, 3
        
        // Skip FP reg saving if they're clean
        bne s0, s1, 1f

        addi sp, sp, -264

        .attribute arch, "rv64imafdc"
        fsd f0, 0(sp)
        fsd f1, 8(sp)
        fsd f2, 16(sp)
        fsd f3, 24(sp)
        fsd f4, 32(sp)
        fsd f5, 40(sp)
        fsd f6, 48(sp)
        fsd f7, 56(sp)
        fsd f8, 64(sp)
        fsd f9, 72(sp)
        fsd f10, 80(sp)
        fsd f11, 88(sp)
        fsd f12, 96(sp)
        fsd f13, 104(sp)
        fsd f14, 112(sp)
        fsd f15, 120(sp)
        fsd f16, 128(sp)
        fsd f17, 136(sp)
        fsd f18, 144(sp)
        fsd f19, 152(sp)
        fsd f20, 160(sp)
        fsd f21, 168(sp)
        fsd f22, 176(sp)
        fsd f23, 184(sp)
        fsd f24, 192(sp)
        fsd f25, 200(sp)
        fsd f26, 208(sp)
        fsd f27, 216(sp)
        fsd f28, 224(sp)
        fsd f29, 232(sp)
        fsd f30, 240(sp)
        fsd f31, 248(sp)
        frcsr t1
        sd t1, 256(sp)
        .attribute arch, "rv64imac"

        li t1, (0b01 << 13)
        csrc sstatus, t1

        // FP registers clean
        1:

        call trap_handler

        // Check FP register status again
        bne s0, s1, 2f

        // Restore if they were dirty
        .attribute arch, "rv64imafdc"
        fld f0, 0(sp)
        fld f1, 8(sp)
        fld f2, 16(sp)
        fld f3, 24(sp)
        fld f4, 32(sp)
        fld f5, 40(sp)
        fld f6, 48(sp)
        fld f7, 56(sp)
        fld f8, 64(sp)
        fld f9, 72(sp)
        fld f10, 80(sp)
        fld f11, 88(sp)
        fld f12, 96(sp)
        fld f13, 104(sp)
        fld f14, 112(sp)
        fld f15, 120(sp)
        fld f16, 128(sp)
        fld f17, 136(sp)
        fld f18, 144(sp)
        fld f19, 152(sp)
        fld f20, 160(sp)
        fld f21, 168(sp)
        fld f22, 176(sp)
        fld f23, 184(sp)
        fld f24, 192(sp)
        fld f25, 200(sp)
        fld f26, 208(sp)
        fld f27, 216(sp)
        fld f28, 224(sp)
        fld f29, 232(sp)
        fld f30, 240(sp)
        fld f31, 248(sp)
        ld t1, 256(sp)
        fscsr t1
        .attribute arch, "rv64imac"

        addi sp, sp, 264

        // FP registers clean
        2:

        // Restore `sepc`
        ld t6, 0(sp)
        csrw sepc, t6

        // Reenable interrupts after sret (set SPIE)
        li t6, 1 << 5
        csrs sstatus, t6

        ld ra, 8(sp)
        // Skip sp for... obvious reasons
        ld gp, 24(sp)
        ld tp, 32(sp)
        ld t0, 40(sp)
        ld t1, 48(sp)
        ld t2, 56(sp)
        ld s0, 64(sp)
        ld s1, 72(sp)
        ld a0, 80(sp)
        ld a1, 88(sp)
        ld a2, 96(sp)
        ld a3, 104(sp)
        ld a4, 112(sp)
        ld a5, 120(sp)
        ld a6, 128(sp)
        ld a7, 136(sp)
        ld s2, 144(sp)
        ld s3, 152(sp)
        ld s4, 160(sp)
        ld s5, 168(sp)
        ld s6, 176(sp)
        ld s7, 184(sp)
        ld s8, 192(sp)
        ld s9, 200(sp)
        ld s10, 208(sp)
        ld s11, 216(sp)
        ld t3, 224(sp)
        ld t4, 232(sp)
        ld t5, 240(sp)
        ld t6, 248(sp)

        // Clear any outstanding atomic reservations
        sc.d zero, zero, 0(sp)

        // Restore `sp`
        ld sp, 16(sp)

        // gtfo
        sret
    "#,
    TRAP_FRAME_SIZE = const { -(core::mem::size_of::<TrapFrame>() as isize) },
    options(noreturn));
}
