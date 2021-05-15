// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    csr::sstatus::TemporaryUserMemoryAccess,
    io::{ConsoleDevice, INPUT_QUEUE},
    mem::paging::{flags, VirtualAddress},
    scheduler::{Scheduler, CURRENT_TASK, SCHEDULER, TASKS},
    task::TaskState,
    trap::TrapFrame,
    utils::{self, Units},
};
use core::convert::TryInto;
use librust::{
    error::{AccessError, KError},
    message::{Message, MessageKind, Recipient, Sender},
    syscalls::Syscall,
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
            let end = start.offset(len);

            log::debug!("Attempting to print memory at {:#p} (len={})", start, len);

            if let Err(addr) = task.memory_manager.is_user_region_valid(start..end, |f| f & flags::READ) {
                log::error!("Bad memory from process >:(");
                return apply_message(KError::InvalidAccess(AccessError::Read(addr.as_ptr())).into(), frame);
            }

            let _guard = TemporaryUserMemoryAccess::new();

            let mut console = crate::io::CONSOLE.lock();
            let bytes = unsafe { core::slice::from_raw_parts(start.as_ptr(), len) };
            for byte in bytes {
                console.write(*byte);
            }

            None.into()
        }
        const { Syscall::ReadStdin as usize } => {
            let start = VirtualAddress::new(msg.arguments[0]);
            let len = msg.arguments[1];
            let end = start.offset(len);

            log::debug!("Attempting to write to memory at {:#p} (len={})", start, len);

            if let Err(addr) = task.memory_manager.is_user_region_valid(start..end, |f| f & flags::WRITE) {
                return apply_message(KError::InvalidAccess(AccessError::Write(addr.as_mut_ptr())).into(), frame);
            }

            let _guard = TemporaryUserMemoryAccess::new();
            let mut n_written = 0;
            for index in 0..len {
                let value = match INPUT_QUEUE.pop() {
                    Some(v) => v,
                    None => break,
                };
                unsafe { start.offset(index).as_mut_ptr().write(value) };
                n_written += 1;
            }

            let mut msg: Message = None.into();
            msg.arguments[0] = n_written;

            msg
        }
        const { Syscall::ReadMessage as usize } => match task.message_queue.pop_front() {
            Some(msg) => msg,
            None => None.into(),
        },
        const { Syscall::AllocMemory as usize } => {
            let size = msg.arguments[0];

            match size {
                0 => KError::InvalidArgument(0).into(),
                _ => {
                    let allocated_at = task.memory_manager.alloc_region(
                        None,
                        utils::round_up_to_next(size, 4096) / 4.kib(),
                        flags::VALID | flags::READ | flags::WRITE | flags::USER,
                        None,
                    );

                    Message {
                        sender: Sender::kernel(),
                        kind: MessageKind::Reply(None),
                        fid: 0,
                        arguments: [allocated_at.as_usize(), 0, 0, 0, 0, 0, 0, 0],
                    }
                }
            }
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
