// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;
pub mod vmspace;

use crate::{
    io::{ConsoleDevice, CLAIMED_DEVICES, INPUT_QUEUE},
    mem::{
        manager::{AddressRegionKind, FillOption, RegionDescription},
        paging::{flags, PageSize, PhysicalAddress, VirtualAddress},
        user::RawUserSlice,
    },
    platform::FDT,
    scheduler::{Scheduler, CURRENT_TASK, SCHEDULER, TASKS},
    task::TaskState,
    trap::TrapFrame,
    utils,
};
use core::{convert::TryInto, num::NonZeroUsize, sync::atomic::Ordering};
use librust::{
    error::{AccessError, KError},
    message::{Message, Recipient, Sender, SyscallRequest, SyscallResult},
    syscalls::{
        allocation::{AllocationOptions, DmaAllocationOptions, MemoryPermissions},
        Syscall,
    },
    task::Tid,
};

pub fn handle(frame: &mut TrapFrame) {
    log::trace!("Handling syscall..");

    let (recipient, message) = get_message(frame);

    match recipient {
        const { Recipient::kernel() } => match do_syscall(message) {
            SyscallResult::Ok((sender, msg)) => apply_message(false, sender, msg, frame),
            SyscallResult::Err(e) => report_error(e, frame),
        },
        _ => match TASKS.get(Tid::new(recipient.value().try_into().unwrap())) {
            Some(task) => {
                let mut task = task.lock();

                if task.state.is_dead() {
                    return report_error(KError::InvalidRecipient, frame);
                }

                log::debug!("Adding message to task (tid: {}): {:?}", recipient.value(), message);

                task.message_queue.push_back((Sender::new(CURRENT_TASK.get().unwrap().value()), message));
                apply_message(false, Sender::kernel(), (), frame);
            }
            None => report_error(KError::InvalidRecipient, frame),
        },
    }
}

fn do_syscall(msg: Message) -> SyscallResult<(Sender, Message), KError> {
    log::trace!("Doing syscall: {:?}", msg);

    let mut sender = Sender::kernel();
    let task_lock = TASKS.get(CURRENT_TASK.get().unwrap()).unwrap();
    let mut task_lock = task_lock.lock();
    let task = &mut *task_lock;

    let syscall_req = SyscallRequest {
        syscall: match Syscall::from_usize(msg.contents[0]) {
            Some(syscall) => syscall,
            None => return SyscallResult::Err(KError::InvalidSyscall(msg.contents[0])),
        },
        arguments: msg.contents[1..].try_into().unwrap(),
    };

    let msg: Message = match syscall_req.syscall {
        Syscall::Exit => {
            log::info!("Active process exited");
            task.state = TaskState::Dead;
            task.message_queue.clear();

            drop(task_lock);

            SCHEDULER.schedule()
        }
        Syscall::Print => {
            let start = VirtualAddress::new(syscall_req.arguments[0]);
            let len = syscall_req.arguments[1];
            let user_slice = RawUserSlice::readable(start, len);
            let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
                Ok(slice) => slice,
                Err((addr, e)) => {
                    log::error!("Bad memory from process: {:?}", e);
                    return SyscallResult::Err(KError::InvalidAccess(AccessError::Read(addr.as_ptr())));
                }
            };

            log::trace!("Attempting to print memory at {:#p} (len={})", start, len);

            let mut console = crate::io::CONSOLE.lock();
            user_slice.with(|bytes| bytes.iter().copied().for_each(|b| console.write(b)));

            Message::default()
        }
        Syscall::ReadStdin => {
            let start = VirtualAddress::new(syscall_req.arguments[0]);
            let len = syscall_req.arguments[1];
            let user_slice = RawUserSlice::writable(start, len);
            let mut user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
                Ok(slice) => slice,
                Err((addr, e)) => {
                    log::error!("Bad memory from process: {:?}", e);
                    return SyscallResult::Err(KError::InvalidAccess(AccessError::Write(addr.as_mut_ptr())));
                }
            };

            log::trace!("Attempting to write to memory at {:#p} (len={})", start, len);

            let mut n_written = 0;
            user_slice.with(|bytes| {
                for byte in bytes {
                    let value = match INPUT_QUEUE.pop() {
                        Some(v) => v,
                        None => break,
                    };
                    *byte = value;
                    n_written += 1;
                }
            });

            Message::from(n_written)
        }
        Syscall::ReadMessage => match task.message_queue.pop_front() {
            Some((sender_, msg)) => {
                sender = sender_;
                msg
            }
            None => return SyscallResult::Err(KError::NoMessages),
        },
        Syscall::AllocVirtualMemory => {
            let size = syscall_req.arguments[0];
            let options = AllocationOptions::new(syscall_req.arguments[1]);
            let permissions = MemoryPermissions::new(syscall_req.arguments[2]);

            if permissions & MemoryPermissions::WRITE && !(permissions & MemoryPermissions::READ) {
                return SyscallResult::Err(KError::InvalidArgument(2));
            }

            let mut flags = flags::VALID | flags::USER;

            if permissions & MemoryPermissions::READ {
                flags |= flags::READ;
            }

            if permissions & MemoryPermissions::WRITE {
                flags |= flags::WRITE;
            }

            if permissions & MemoryPermissions::EXECUTE {
                flags |= flags::EXECUTE;
            }

            let page_size =
                if options & AllocationOptions::LargePage { PageSize::Megapage } else { PageSize::Kilopage };

            match size {
                0 => return SyscallResult::Err(KError::InvalidArgument(0)),
                _ => {
                    let allocated_at = task.memory_manager.alloc_region(
                        None,
                        RegionDescription {
                            size: page_size,
                            len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                            contiguous: false,
                            flags,
                            fill: if options & AllocationOptions::Zero {
                                FillOption::Zeroed
                            } else {
                                FillOption::Unitialized
                            },
                            kind: AddressRegionKind::UserAllocated,
                        },
                    );

                    log::debug!("Allocated memory at {:#p} for user process", allocated_at.start);

                    Message::from(allocated_at.start.as_usize())
                }
            }
        }
        Syscall::GetTid => (CURRENT_TASK.get().unwrap().value()).into(),
        Syscall::CreateChannelMessage => {
            Message::from(channel::create_message(task, syscall_req.arguments[0], syscall_req.arguments[1])?)
        }
        Syscall::SendChannelMessage => Message::from(channel::send_message(
            task,
            syscall_req.arguments[0],
            syscall_req.arguments[1],
            syscall_req.arguments[2],
        )?),
        Syscall::ReadChannel => Message::from(channel::read_message(task, syscall_req.arguments[0])?),
        Syscall::RetireChannelMessage => {
            Message::from(channel::retire_message(task, syscall_req.arguments[0], syscall_req.arguments[1])?)
        }
        Syscall::AllocDmaMemory => {
            let size = syscall_req.arguments[0];
            let options = DmaAllocationOptions::new(syscall_req.arguments[1]);
            let page_size = PageSize::Kilopage;

            match size {
                0 => return SyscallResult::Err(KError::InvalidArgument(0)),
                _ => {
                    let allocated_at = task.memory_manager.alloc_region(
                        None,
                        RegionDescription {
                            size: page_size,
                            len: utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                            contiguous: true,
                            flags: flags::VALID | flags::USER | flags::READ | flags::WRITE,
                            fill: if options & DmaAllocationOptions::ZERO {
                                FillOption::Zeroed
                            } else {
                                FillOption::Unitialized
                            },
                            kind: AddressRegionKind::Dma,
                        },
                    );

                    let phys = task.memory_manager.resolve(allocated_at.start).unwrap();

                    log::debug!("Allocated DMA memory at {:#p} for user process", allocated_at.start);

                    Message::from((phys.as_usize(), allocated_at.start.as_usize()))
                }
            }
        }
        Syscall::CreateVmspace => Message::from(vmspace::create_vmspace(task)?),
        Syscall::AllocVmspaceObject => Message::from(vmspace::alloc_vmspace_object(
            task,
            syscall_req.arguments[0],
            syscall_req.arguments[1],
            syscall_req.arguments[2],
            syscall_req.arguments[3],
        )?),
        Syscall::SpawnVmspace => Message::from(vmspace::spawn_vmspace(
            task,
            syscall_req.arguments[0],
            syscall_req.arguments[1],
            syscall_req.arguments[2],
            syscall_req.arguments[3],
            syscall_req.arguments[4],
            syscall_req.arguments[5],
            syscall_req.arguments[6],
        )?),
        Syscall::ClaimDevice => {
            let start = VirtualAddress::new(syscall_req.arguments[0]);
            let len = syscall_req.arguments[1];
            let user_slice = RawUserSlice::readable(start, len);
            let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
                Ok(slice) => slice,
                Err((addr, e)) => {
                    log::error!("Bad memory from process: {:?}", e);
                    return SyscallResult::Err(KError::InvalidAccess(AccessError::Read(addr.as_ptr())));
                }
            };

            let slice = user_slice.guarded();
            let node_path = match core::str::from_utf8(&slice) {
                Ok(s) => s,
                Err(_) => {
                    log::error!("Invalid UTF-8 in FDT node name from process");
                    return SyscallResult::Err(KError::InvalidArgument(0));
                }
            };

            // FIXME: make better errors
            let claimed = CLAIMED_DEVICES.read();
            if claimed.get(node_path).is_some() {
                return SyscallResult::Err(KError::InvalidArgument(0));
            }

            let fdt = unsafe { fdt::Fdt::from_ptr(FDT.load(Ordering::Acquire)) }.unwrap();

            // FIXME: probably should add some sanity checks for what we're
            // mapping
            match fdt.find_node(node_path) {
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

                            Message::from((map_to.start.as_usize(), len))
                        }
                        _ => return SyscallResult::Err(KError::InvalidArgument(0)),
                    }
                }
                None => return SyscallResult::Err(KError::InvalidArgument(0)),
            }
        }
    };

    SyscallResult::Ok((sender, msg))
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

fn apply_message<T: Into<Message>>(is_err: bool, sender: Sender, msg: T, frame: &mut TrapFrame) {
    frame.registers.t0 = is_err as usize;
    frame.registers.t1 = sender.value();

    let msg = msg.into();
    frame.registers.t2 = msg.contents[0];
    frame.registers.t3 = msg.contents[1];
    frame.registers.t4 = msg.contents[2];
    frame.registers.t5 = msg.contents[3];
    frame.registers.t6 = msg.contents[4];
    frame.registers.a0 = msg.contents[5];
    frame.registers.a1 = msg.contents[6];
    frame.registers.a2 = msg.contents[7];
    frame.registers.a3 = msg.contents[8];
    frame.registers.a4 = msg.contents[9];
    frame.registers.a5 = msg.contents[10];
    frame.registers.a6 = msg.contents[11];
    frame.registers.a7 = msg.contents[12];
}

fn report_error<T: Into<Message>>(error: T, frame: &mut TrapFrame) {
    apply_message(true, Sender::kernel(), error, frame)
}
