// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;
pub mod mem;
pub mod misc;
pub mod vmspace;

use crate::{
    capabilities::{Capability, CapabilityResource},
    interrupts::PLIC,
    io::CLAIMED_DEVICES,
    mem::{
        paging::{PhysicalAddress, VirtualAddress},
        user::RawUserSlice,
    },
    platform::FDT,
    scheduler::{Scheduler, WakeToken, CURRENT_TASK, SCHEDULER, TASKS},
    task::{Task, TaskState},
    trap::{GeneralRegisters, TrapFrame},
    HART_ID,
};
use core::{convert::TryInto, sync::atomic::Ordering};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::{AccessError, KError},
    message::{KernelNotification, Message, Recipient, Sender, SyscallRequest},
    syscalls::{
        allocation::{AllocationOptions, DmaAllocationOptions, MemoryPermissions},
        channel::MessageId,
        vmspace::VmspaceObjectId,
        Syscall,
    },
    task::Tid,
};

#[derive(Debug)]
pub enum SyscallOutcome {
    Processed(Message),
    Err(KError),
    Block,
    Kill,
}

impl SyscallOutcome {
    pub fn processed<T: Into<Message>>(t: T) -> Self {
        Self::Processed(t.into())
    }
}

// :(
pub fn handle(frame: &mut TrapFrame, sepc: usize) -> usize {
    log::trace!("Handling syscall..");

    let (recipient, message) = get_message(frame);
    let task_lock = TASKS.active_on_cpu().unwrap();
    let mut task_lock = task_lock.lock();
    let task = &mut *task_lock;

    match recipient {
        const { Recipient::kernel() } => {
            match do_syscall(task, message) {
                (sender, SyscallOutcome::Processed(message)) => {
                    apply_message(false, sender, message, &mut frame.registers)
                }
                (_, SyscallOutcome::Err(e)) => report_error(e, &mut frame.registers),
                (_, SyscallOutcome::Block) => {
                    let tid = CURRENT_TASK.get().unwrap();
                    log::debug!("Blocking task {:?}", task.name);
                    task.context.gp_regs = frame.registers;

                    // Don't re-call the syscall after its unblocked
                    task.context.pc = sepc + 4;

                    drop(task_lock);
                    SCHEDULER.block(tid);
                    SCHEDULER.schedule()
                }
                (_, SyscallOutcome::Kill) => {
                    task.state = TaskState::Dead;

                    drop(task_lock);
                    SCHEDULER.schedule()
                }
            }
        }
        _ => match TASKS.get(Tid::new(recipient.value().try_into().unwrap())) {
            Some(task) => {
                let mut task = task.lock();

                if task.state.is_dead() {
                    report_error(KError::InvalidRecipient, &mut frame.registers);
                } else {
                    log::debug!("Adding message to task (tid: {}): {:?}", recipient.value(), message);

                    task.message_queue.push(Sender::new(CURRENT_TASK.get().unwrap().value()), message);
                    apply_message(false, Sender::kernel(), (), &mut frame.registers);
                }
            }
            None => report_error(KError::InvalidRecipient, &mut frame.registers),
        },
    }

    task.context.gp_regs = frame.registers;
    sepc + 4
}

fn do_syscall(task: &mut Task, msg: Message) -> (Sender, SyscallOutcome) {
    log::trace!("Doing syscall: {:?}", msg);

    let mut sender = Sender::kernel();

    let syscall_req = SyscallRequest {
        syscall: match Syscall::from_usize(msg.contents[0]) {
            Some(syscall) => syscall,
            None => return (Sender::kernel(), SyscallOutcome::Err(KError::InvalidSyscall(msg.contents[0]))),
        },
        arguments: msg.contents[1..].try_into().unwrap(),
    };

    let outcome: SyscallOutcome = match syscall_req.syscall {
        Syscall::Exit => {
            log::debug!("Active process {:?} exited", task.name);
            return (Sender::kernel(), SyscallOutcome::Kill);
        }
        Syscall::Print => misc::print(task, VirtualAddress::new(syscall_req.arguments[0]), syscall_req.arguments[1]),
        Syscall::ReadStdin => {
            misc::read_stdin(task, VirtualAddress::new(syscall_req.arguments[0]), syscall_req.arguments[1])
        }
        Syscall::ReadMessage => match task.message_queue.pop() {
            Some((sender_, msg)) => {
                log::debug!("Message read for task {}", task.name);
                sender = sender_;
                SyscallOutcome::Processed(msg)
            }
            None => {
                log::debug!("Registering wake for read_message");
                task.message_queue.register_wake(WakeToken::new(CURRENT_TASK.get().unwrap(), |task| {
                    log::debug!("Waking task for read_message");
                    task.state = TaskState::Running;
                    let (sender, message) = task.message_queue.pop().expect("woken but no messages in queue?");
                    apply_message(false, sender, message, &mut task.context.gp_regs);
                }));
                SyscallOutcome::Block
            }
        },
        Syscall::AllocVirtualMemory => mem::alloc_virtual_memory(
            task,
            syscall_req.arguments[0],
            AllocationOptions::new(syscall_req.arguments[1]),
            MemoryPermissions::new(syscall_req.arguments[2]),
        ),
        Syscall::GetTid => SyscallOutcome::processed(CURRENT_TASK.get().unwrap().value()),
        Syscall::CreateChannelMessage => {
            channel::create_message(task, CapabilityPtr::new(syscall_req.arguments[0]), syscall_req.arguments[1])
        }
        Syscall::SendChannelMessage => channel::send_message(
            task,
            CapabilityPtr::new(syscall_req.arguments[0]),
            MessageId::new(syscall_req.arguments[1]),
            syscall_req.arguments[2],
            RawUserSlice::new(VirtualAddress::new(syscall_req.arguments[3]), syscall_req.arguments[4]),
        ),
        Syscall::ReadChannel => channel::read_message(
            task,
            CapabilityPtr::new(syscall_req.arguments[0]),
            RawUserSlice::new(VirtualAddress::new(syscall_req.arguments[1]), syscall_req.arguments[2]),
        ),
        Syscall::RetireChannelMessage => channel::retire_message(
            task,
            CapabilityPtr::new(syscall_req.arguments[0]),
            MessageId::new(syscall_req.arguments[1]),
        ),
        Syscall::AllocDmaMemory => {
            mem::alloc_dma_memory(task, syscall_req.arguments[0], DmaAllocationOptions::new(syscall_req.arguments[1]))
        }
        Syscall::CreateVmspace => vmspace::create_vmspace(task),
        Syscall::QueryMemoryCapability => mem::query_mem_cap(task, CapabilityPtr::new(syscall_req.arguments[0])),
        Syscall::AllocVmspaceObject => vmspace::alloc_vmspace_object(
            task,
            syscall_req.arguments[0],
            syscall_req.arguments[1],
            syscall_req.arguments[2],
            syscall_req.arguments[3],
        ),
        Syscall::SpawnVmspace => vmspace::spawn_vmspace(
            task,
            VmspaceObjectId::new(syscall_req.arguments[0]),
            VirtualAddress::new(syscall_req.arguments[1]),
            syscall_req.arguments[2],
            syscall_req.arguments[3],
            syscall_req.arguments[4],
            syscall_req.arguments[5],
            syscall_req.arguments[6],
            syscall_req.arguments[7],
            syscall_req.arguments[8],
        ),
        Syscall::ClaimDevice => {
            let start = VirtualAddress::new(syscall_req.arguments[0]);
            let len = syscall_req.arguments[1];
            let user_slice = RawUserSlice::readable(start, len);
            let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
                Ok(slice) => slice,
                Err((addr, e)) => {
                    log::error!("Bad memory from process: {:?}", e);
                    return (
                        Sender::kernel(),
                        SyscallOutcome::Err(KError::InvalidAccess(AccessError::Read(addr.as_ptr()))),
                    );
                }
            };

            let slice = user_slice.guarded();
            let node_path = match core::str::from_utf8(&slice) {
                Ok(s) => s,
                Err(_) => {
                    log::error!("Invalid UTF-8 in FDT node name from process");
                    return (Sender::kernel(), SyscallOutcome::Err(KError::InvalidArgument(0)));
                }
            };

            // FIXME: make better errors
            let claimed = CLAIMED_DEVICES.read();
            if claimed.get(node_path).is_some() {
                return (Sender::kernel(), SyscallOutcome::Err(KError::InvalidArgument(0)));
            }

            let fdt = unsafe { fdt::Fdt::from_ptr(FDT.load(Ordering::Acquire)) }.unwrap();

            // FIXME: probably should add some sanity checks for what we're
            // mapping
            //
            // FIXME: `fdt` needs updated so that we can get the full node path,
            // so work around that temporarily here
            let mut all_nodes = fdt.all_nodes();
            match all_nodes.find(|n| n.name == node_path) {
                Some(node) => {
                    // FIXME: what about multiple regions?
                    match node.reg().into_iter().flatten().next() {
                        Some(fdt::standard_nodes::MemoryRegion { size: Some(len), starting_address }) => {
                            claimed.upgrade().insert(node_path.into(), CURRENT_TASK.get().unwrap());
                            let map_to = unsafe {
                                task.memory_manager.map_mmio_device(
                                    PhysicalAddress::from_ptr(starting_address),
                                    None,
                                    len,
                                )
                            };

                            // FIXME: this probably needs marked as `Clone` in
                            // `fdt` or something
                            let interrupts = node.interrupts().into_iter().flatten();
                            let cptr = task.cspace.mint(Capability {
                                resource: CapabilityResource::Mmio(map_to, interrupts.collect()),
                                rights: CapabilityRights::GRANT | CapabilityRights::READ | CapabilityRights::WRITE,
                            });

                            let current_tid = CURRENT_TASK.get().unwrap();
                            let interrupts = node.interrupts().into_iter().flatten();

                            let plic = PLIC.lock();
                            let plic = plic.as_ref().unwrap();
                            for interrupt in interrupts {
                                log::debug!("Giving interrupt {} to task {}", interrupt, task.name);
                                plic.enable_interrupt(crate::platform::current_plic_context(), interrupt);
                                plic.set_context_threshold(crate::platform::current_plic_context(), 0);
                                plic.set_interrupt_priority(interrupt, 7);
                                crate::interrupts::isr::register_isr(interrupt, move |plic, _, id| {
                                    plic.disable_interrupt(crate::platform::current_plic_context(), id);
                                    let task = TASKS.get(current_tid).unwrap();
                                    let mut task = task.lock();

                                    log::debug!(
                                        "Interrupt {} triggered (hart: {}), notifying task {}",
                                        id,
                                        HART_ID.get(),
                                        task.name
                                    );

                                    task.claimed_interrupts.insert(id, HART_ID.get());
                                    task.message_queue.push(
                                        Sender::kernel(),
                                        Message::from(KernelNotification::InterruptOccurred(id)),
                                    );

                                    Ok(())
                                });
                            }

                            SyscallOutcome::processed(cptr.value())
                        }
                        _ => return (Sender::kernel(), SyscallOutcome::Err(KError::InvalidArgument(0))),
                    }
                }
                None => return (Sender::kernel(), SyscallOutcome::Err(KError::InvalidArgument(0))),
            }
        }
        Syscall::CompleteInterrupt => {
            let interrupt_id = syscall_req.arguments[0];
            match task.claimed_interrupts.remove(&interrupt_id) {
                None => SyscallOutcome::Err(KError::InvalidArgument(0)),
                Some(hart) => {
                    log::debug!("Task {} completing interrupt {}", task.name, interrupt_id);
                    if let Some(plic) = &*PLIC.lock() {
                        plic.complete(crate::platform::plic_context_for(hart), interrupt_id);
                        plic.enable_interrupt(crate::platform::plic_context_for(hart), interrupt_id);
                    }

                    SyscallOutcome::processed(())
                }
            }
        }
        Syscall::QueryMmioCapability => mem::query_mmio_cap(task, CapabilityPtr::new(syscall_req.arguments[0])),
    };

    (sender, outcome)
}

fn get_message(frame: &TrapFrame) -> (Recipient, Message) {
    let mut contents = [0; 13];

    let recipient = Recipient::new(frame.registers.t0);
    contents[0] = frame.registers.t2;
    contents[1] = frame.registers.t3;
    contents[2] = frame.registers.t4;
    contents[3] = frame.registers.t5;
    contents[4] = frame.registers.t6;
    contents[5] = frame.registers.a0;
    contents[6] = frame.registers.a1;
    contents[7] = frame.registers.a2;
    contents[8] = frame.registers.a3;
    contents[9] = frame.registers.a4;
    contents[10] = frame.registers.a5;
    contents[11] = frame.registers.a6;
    contents[12] = frame.registers.a7;

    (recipient, Message { contents })
}

fn apply_message<T: Into<Message>>(is_err: bool, sender: Sender, msg: T, frame: &mut GeneralRegisters) {
    frame.t0 = is_err as usize;
    frame.t1 = sender.value();

    let msg = msg.into();
    frame.t2 = msg.contents[0];
    frame.t3 = msg.contents[1];
    frame.t4 = msg.contents[2];
    frame.t5 = msg.contents[3];
    frame.t6 = msg.contents[4];
    frame.a0 = msg.contents[5];
    frame.a1 = msg.contents[6];
    frame.a2 = msg.contents[7];
    frame.a3 = msg.contents[8];
    frame.a4 = msg.contents[9];
    frame.a5 = msg.contents[10];
    frame.a6 = msg.contents[11];
    frame.a7 = msg.contents[12];
}

fn report_error<T: Into<Message>>(error: T, frame: &mut GeneralRegisters) {
    apply_message(true, Sender::kernel(), error, frame)
}
