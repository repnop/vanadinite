// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;

use crate::{
    io::{ConsoleDevice, INPUT_QUEUE},
    mem::{
        manager::{AddressRegionKind, FillOption},
        paging::{flags, PageSize, VirtualAddress},
        user::RawUserSlice,
    },
    scheduler::{Scheduler, CURRENT_TASK, SCHEDULER, TASKS},
    task::TaskState,
    trap::TrapFrame,
    utils,
};
use core::{convert::TryInto, num::NonZeroUsize};
use librust::{
    error::{AccessError, KError},
    message::{Message, MessageKind, Recipient, Sender},
    syscalls::{
        allocation::{AllocationOptions, MemoryPermissions},
        Syscall,
    },
    task::Tid,
};

pub fn handle(frame: &mut TrapFrame) {
    log::debug!("Handling syscall..");

    let recipient = Recipient::new(frame.registers.t0);
    let message = match parse_message(frame) {
        Some(mut msg) => {
            msg.sender = Sender::task(CURRENT_TASK.get().unwrap());
            msg
        }
        None => {
            log::debug!("Bad message from userspace");
            return apply_message(KError::InvalidMessage.into(), frame);
        }
    };

    match recipient {
        const { Recipient::kernel() } => do_syscall(message, frame),
        _ => match TASKS.get(Tid::new(recipient.value().try_into().unwrap())) {
            Some(task) => {
                let mut task = task.lock();

                if task.state.is_dead() {
                    return apply_message(KError::InvalidRecipient.into(), frame);
                }

                log::debug!("Adding message to task (tid: {}): {:?}", recipient.value(), message);

                task.message_queue.push_back(message);
                apply_message(None.into(), frame);
            }
            None => apply_message(KError::InvalidRecipient.into(), frame),
        },
    }
}

fn do_syscall(msg: Message, frame: &mut TrapFrame) {
    log::debug!("Doing syscall: {:?}", msg);

    if let MessageKind::ApplicationSpecific(_) | MessageKind::Reply(_) = msg.kind {
        return apply_message(KError::InvalidMessage.into(), frame);
    }

    let task_lock = TASKS.get(CURRENT_TASK.get().unwrap()).unwrap();
    let mut task_lock = task_lock.lock();
    let task = &mut *task_lock;

    let mut msg = match msg.fid {
        const { Syscall::Exit as usize } => {
            log::info!("Active process exited");
            task.state = TaskState::Dead;
            task.message_queue.clear();

            drop(task_lock);

            SCHEDULER.schedule()
        }
        const { Syscall::Print as usize } => {
            let start = VirtualAddress::new(msg.arguments[0]);
            let len = msg.arguments[1];
            let user_slice = RawUserSlice::readable(start, len);
            let user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
                Ok(slice) => slice,
                Err((addr, e)) => {
                    log::error!("Bad memory from process: {:?}", e);
                    return apply_message(KError::InvalidAccess(AccessError::Read(addr.as_ptr())).into(), frame);
                }
            };

            log::debug!("Attempting to print memory at {:#p} (len={})", start, len);

            let mut console = crate::io::CONSOLE.lock();
            user_slice.with(|bytes| bytes.iter().copied().for_each(|b| console.write(b)));

            None.into()
        }
        const { Syscall::ReadStdin as usize } => {
            let start = VirtualAddress::new(msg.arguments[0]);
            let len = msg.arguments[1];
            let user_slice = RawUserSlice::writable(start, len);
            let mut user_slice = match unsafe { user_slice.validate(&task.memory_manager) } {
                Ok(slice) => slice,
                Err((addr, e)) => {
                    log::error!("Bad memory from process: {:?}", e);
                    return apply_message(KError::InvalidAccess(AccessError::Write(addr.as_mut_ptr())).into(), frame);
                }
            };

            log::debug!("Attempting to write to memory at {:#p} (len={})", start, len);

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

            let mut msg: Message = None.into();
            msg.arguments[0] = n_written;

            msg
        }
        const { Syscall::ReadMessage as usize } => {
            // Avoid accidentally overwriting sender later on
            return apply_message(
                match task.message_queue.pop_front() {
                    Some(msg) => msg,
                    None => {
                        let mut msg: Message = None.into();
                        msg.sender = Sender::kernel();

                        msg
                    }
                },
                frame,
            );
        }
        const { Syscall::AllocVirtualMemory as usize } => loop {
            let size = msg.arguments[0];
            let options = AllocationOptions::new(msg.arguments[1]);
            let permissions = MemoryPermissions::new(msg.arguments[2]);

            if permissions & MemoryPermissions::Write && !(permissions & MemoryPermissions::Read) {
                break KError::InvalidArgument(2).into();
            }

            let mut flags = flags::VALID | flags::USER;

            if permissions & MemoryPermissions::Read {
                flags |= flags::READ;
            }

            if permissions & MemoryPermissions::Write {
                flags |= flags::WRITE;
            }

            if permissions & MemoryPermissions::Execute {
                flags |= flags::EXECUTE;
            }

            let page_size =
                if options & AllocationOptions::LargePage { PageSize::Megapage } else { PageSize::Kilopage };

            break match size {
                0 => KError::InvalidArgument(0).into(),
                _ => {
                    let allocated_at = task.memory_manager.alloc_region(
                        None,
                        page_size,
                        utils::round_up_to_next(size, page_size.to_byte_size()) / page_size.to_byte_size(),
                        flags,
                        if options & AllocationOptions::Zero { FillOption::Zeroed } else { FillOption::Unitialized },
                        AddressRegionKind::UserAllocated,
                    );

                    log::debug!("Allocated memory at {:#p} for user process", allocated_at.start);

                    Message {
                        sender: Sender::kernel(),
                        kind: MessageKind::Reply(None),
                        fid: 0,
                        arguments: [allocated_at.start.as_usize(), 0, 0, 0, 0, 0, 0, 0],
                    }
                }
            };
        },
        const { Syscall::GetTid as usize } => Message {
            sender: Sender::kernel(),
            kind: MessageKind::Reply(None),
            fid: 0,
            arguments: [CURRENT_TASK.get().unwrap().value(), 0, 0, 0, 0, 0, 0, 0],
        },
        const { Syscall::CreateChannel as usize } => loop {
            let tid = match NonZeroUsize::new(msg.arguments[0]) {
                Some(tid) => tid,
                None => break KError::InvalidArgument(0).into(),
            };

            break channel::create_channel(task, Tid::new(tid)).into();
        },
        const { Syscall::CreateChannelMessage as usize } => {
            channel::create_message(task, msg.arguments[0], msg.arguments[1]).into()
        }
        const { Syscall::SendChannelMessage as usize } => {
            channel::send_message(task, msg.arguments[0], msg.arguments[1], msg.arguments[2]).into()
        }
        const { Syscall::ReadChannel as usize } => channel::read_message(task, msg.arguments[0]).into(),
        const { Syscall::RetireChannelMessage as usize } => {
            channel::retire_message(task, msg.arguments[0], msg.arguments[1]).into()
        }
        id => KError::InvalidSyscall(id).into(),
    };

    msg.sender = Sender::kernel();

    log::debug!("Replying with {:x?}", msg);

    apply_message(msg, frame)
}

fn parse_message(frame: &TrapFrame) -> Option<Message> {
    Some(Message {
        sender: Sender::task(CURRENT_TASK.get().unwrap()),
        kind: MessageKind::from_parts(frame.registers.t2, frame.registers.t3)?,
        fid: frame.registers.t4,
        arguments: [
            frame.registers.a0,
            frame.registers.a1,
            frame.registers.a2,
            frame.registers.a3,
            frame.registers.a4,
            frame.registers.a5,
            frame.registers.a6,
            frame.registers.a7,
        ],
    })
}

fn apply_message(msg: Message, frame: &mut TrapFrame) {
    frame.registers.t1 = msg.sender.value();
    (frame.registers.t2, frame.registers.t3) = msg.kind.into_parts();
    frame.registers.t4 = msg.fid;

    frame.registers.a0 = msg.arguments[0];
    frame.registers.a1 = msg.arguments[1];
    frame.registers.a2 = msg.arguments[2];
    frame.registers.a3 = msg.arguments[3];
    frame.registers.a4 = msg.arguments[4];
    frame.registers.a5 = msg.arguments[5];
    frame.registers.a6 = msg.arguments[6];
    frame.registers.a7 = msg.arguments[7];
}