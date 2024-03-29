// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    capabilities::{Capability, CapabilityResource},
    interrupts::PLIC,
    mem::{
        paging::{flags::Flags, VirtualAddress},
        user::{self, RawUserSlice},
    },
    scheduler::{waitqueue::WaitQueue, TASKS},
    sync::SpinMutex,
    task::Task,
    trap::GeneralRegisters,
    utils::SameHartDeadlockDetection,
    HART_ID,
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicBool, Ordering};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::{
        channel::{ChannelReadFlags, KernelMessage},
        mem::MemoryPermissions,
    },
    task::Tid,
};

#[derive(Debug, Clone)]
pub struct UserspaceChannel {
    pub(super) sender: Sender,
    pub(super) receiver: Receiver,
}

impl UserspaceChannel {
    pub fn new() -> (Self, Self) {
        let (sender1, receiver1) = {
            let message_queue = Arc::new(SpinMutex::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let waitqueue = Arc::new(WaitQueue::new());

            let sender = Sender {
                inner: Arc::clone(&message_queue),
                alive: Arc::clone(&alive),
                waitqueue: Arc::clone(&waitqueue),
                other_tid: None,
                other_cptr: CapabilityPtr::new(usize::MAX),
            };
            let receiver = Receiver { inner: message_queue, alive, waitqueue };

            (sender, receiver)
        };

        let (sender2, receiver2) = {
            let message_queue = Arc::new(SpinMutex::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let waitqueue = Arc::new(WaitQueue::new());

            let sender = Sender {
                inner: Arc::clone(&message_queue),
                alive: Arc::clone(&alive),
                waitqueue: Arc::clone(&waitqueue),
                other_tid: None,
                other_cptr: CapabilityPtr::new(usize::MAX),
            };
            let receiver = Receiver { inner: message_queue, alive, waitqueue };

            (sender, receiver)
        };

        let first = Self { sender: sender1, receiver: receiver2 };
        let second = Self { sender: sender2, receiver: receiver1 };

        (first, second)
    }
}

#[derive(Debug)]
pub struct ChannelMessage {
    pub data: [usize; 7],
    pub caps: Vec<Capability>,
}

#[derive(Debug, Clone)]
pub(super) struct Receiver {
    // FIXME: Replace these with something like a lockfree ring buffer
    pub(super) inner: Arc<SpinMutex<VecDeque<ChannelMessage>, SameHartDeadlockDetection>>,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) waitqueue: Arc<WaitQueue>,
}

impl Receiver {
    pub fn recv(&self) -> Result<ChannelMessage, ()> {
        loop {
            let mut lock = self.inner.lock();
            let msg = lock.pop_front();
            match msg {
                Some(message) => break Ok(message),
                // FIXME: should we check `alive` here?
                None => self.waitqueue.wait(move || drop(lock)),
            }
        }
    }

    pub fn try_recv(&self) -> Result<Option<ChannelMessage>, ()> {
        match self.inner.lock().pop_front() {
            Some(message) => Ok(Some(message)),
            None => match self.alive.load(Ordering::Acquire) {
                true => Ok(None),
                false => Err(()),
            },
        }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        // FIXME: this currently breaks sending messages...
        // self.alive.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone)]
pub struct Sender {
    // FIXME: Replace these with something like a lockfree ring buffer
    pub(super) inner: Arc<SpinMutex<VecDeque<ChannelMessage>, SameHartDeadlockDetection>>,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) waitqueue: Arc<WaitQueue>,
    pub(super) other_tid: Option<Tid>,
    pub(super) other_cptr: CapabilityPtr,
}

impl Sender {
    #[track_caller]
    pub fn send(&self, message: ChannelMessage) -> Result<(), ChannelMessage> {
        if !self.alive.load(Ordering::Acquire) {
            log::debug!("Channel to {:?}:{:?} is dead", self.other_tid, self.other_cptr);
            return Err(message);
        }

        // FIXME: set a buffer limit at some point
        let mut lock = self.inner.lock();

        if let Some(task) = self.other_tid.and_then(|tid| TASKS.get(tid)) {
            log::debug!("Enqueuing kernel message for other cptr [{}:{:?}]", task.name, self.other_cptr);
            let task_state = task.mutable_state.lock();
            if task_state.subscribes_to_events {
                let sender = task_state.kernel_channel.sender.clone();
                drop(task_state);
                sender.send(ChannelMessage {
                    data: KernelMessage::into_parts(KernelMessage::NewChannelMessage(self.other_cptr)),
                    caps: Vec::new(),
                })?;
            }
        }

        lock.push_back(message);
        self.waitqueue.wake_one();

        Ok(())
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        // FIXME: this currently breaks sending messages
        // self.alive.store(false, Ordering::Release);
    }
}

pub fn send_message(task: &Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let task_state = task.mutable_state.lock();

    let cptr = CapabilityPtr::new(frame.a1);
    let caps =
        RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(frame.a2), frame.a3);
    let data = [frame.t0, frame.t1, frame.t2, frame.t3, frame.t4, frame.t5, frame.t6];

    let channel = match task_state.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel.clone()
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match caps.len() {
        0 => Vec::new(),
        _ => {
            let cap_slice = match unsafe { caps.validate(&task_state.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return Err(SyscallError::InvalidArgument(3)),
            };

            let cap_slice = cap_slice.guarded();

            // NOTE: A capacity of 2 is used to prevent users from passing us a
            // (potentially very) large slice of invalid cptrs and causing us to
            // pre-allocate a large amount of memory that will only potentially
            // cause heap allocator pressure. Messages are unlikely to contain
            // more than 1 or 2 caps, so default to 2 as a reasonable
            // preallocation amount.
            let mut cloned_caps = Vec::with_capacity(2);
            for librust::capabilities::Capability { cptr, rights } in cap_slice.iter().copied() {
                match task_state.cspace.resolve(cptr) {
                    Some(cap) if cap.rights.is_superset(rights) && cap.rights & CapabilityRights::GRANT => {
                        // Can't allow sending invalid memory permissions
                        if let CapabilityResource::SharedMemory(..) = &cap.resource {
                            if cap.rights & CapabilityRights::WRITE && !(cap.rights & CapabilityRights::READ) {
                                return Err(SyscallError::InvalidArgument(2));
                            }
                        }

                        cloned_caps.push(cap.clone())
                    }
                    _ => return Err(SyscallError::InvalidArgument(2)),
                }
            }

            cloned_caps
        }
    };

    log::debug!("[{}:{}] Sending channel message", task.name, task.tid);
    drop(task_state);
    // FIXME: this should notify the sender the channel is dead if it is
    channel.sender.send(ChannelMessage { data, caps }).unwrap();

    Ok(())
}

pub fn read_message(task: &Task, regs: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let task_state = task.mutable_state.lock();

    let cptr = CapabilityPtr::new(regs.a1);
    let cap_buffer = RawUserSlice::<user::ReadWrite, librust::capabilities::CapabilityWithDescription>::new(
        VirtualAddress::new(regs.a2),
        regs.a3,
    );
    let flags = ChannelReadFlags::new(regs.a4);

    let channel = match task_state.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel.clone()
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // FIXME: this probably needs the lock to make sure a message wasn't sent
    // after the check but before the register

    // FIXME: check for broken channel

    drop(task_state);
    let ChannelMessage { data, mut caps } = if flags & ChannelReadFlags::NONBLOCKING {
        match channel.receiver.try_recv() {
            Ok(Some(msg)) => msg,
            Ok(None) => return Err(SyscallError::WouldBlock),
            Err(e) => todo!("handle channel read error: {e:?}"),
        }
    } else {
        match channel.receiver.recv() {
            Ok(msg) => msg,
            Err(e) => todo!("handle channel read error: {e:?}"),
        }
    };

    let mut task_state = task.mutable_state.lock();

    let (caps_written, caps_remaining) = match cap_buffer.len() {
        0 => (0, caps.len()),
        len => {
            let cap_slice = match unsafe { cap_buffer.validate(&task_state.memory_manager) } {
                Ok(cap_slice) => cap_slice,
                Err(_) => return Err(SyscallError::InvalidArgument(3)),
            };

            let n_caps_to_write = len.min(caps.len());
            let mut cap_slice = cap_slice.guarded();
            for (target, cap) in cap_slice.iter_mut().zip(caps.drain(..n_caps_to_write)) {
                let rights = cap.rights;
                let (cptr, description) = match cap.resource {
                    CapabilityResource::Channel(channel) => {
                        let other_tid = match channel.sender.other_tid {
                            Some(tid) if task.tid != tid => tid,
                            Some(_) => continue,
                            None => {
                                log::warn!("Channel cap sent but didn't contain TID for other side?");
                                continue;
                            }
                        };

                        let other_task = match TASKS.get(other_tid) {
                            Some(task) => task,
                            None => continue, // Task is... gone? hmm..
                        };

                        let (mut c1, mut c2) = UserspaceChannel::new();
                        c1.sender.other_tid = Some(task.tid);
                        c2.sender.other_tid = Some(other_tid);

                        let mut other_task = other_task.mutable_state.lock();
                        let cptr = task_state.cspace.mint_with(|this_cptr| {
                            other_task.cspace.mint_with(|other_cptr| {
                                c1.sender.other_cptr = this_cptr;
                                c2.sender.other_cptr = other_cptr;
                                Capability { resource: CapabilityResource::Channel(c1), rights }
                            });

                            Capability { resource: CapabilityResource::Channel(c2), rights }
                        });

                        (cptr, librust::capabilities::CapabilityDescription::Channel)
                    }
                    CapabilityResource::SharedMemory(region, _, kind) => {
                        let mut permissions = MemoryPermissions::new(0);
                        let mut memflags = Flags::VALID | Flags::USER;

                        if rights & CapabilityRights::READ {
                            permissions |= MemoryPermissions::READ;
                            memflags |= Flags::READ;
                        }

                        if rights & CapabilityRights::WRITE {
                            permissions |= MemoryPermissions::WRITE;
                            memflags |= Flags::WRITE;
                        }

                        if rights & CapabilityRights::EXECUTE {
                            permissions |= MemoryPermissions::EXECUTE;
                            memflags |= Flags::EXECUTE;
                        }

                        let addr = task_state.memory_manager.apply_shared_region(None, memflags, region.clone(), kind);

                        let cptr = task_state.cspace.mint(Capability {
                            resource: CapabilityResource::SharedMemory(region, addr.clone(), kind),
                            rights,
                        });

                        (
                            cptr,
                            librust::capabilities::CapabilityDescription::Memory {
                                ptr: addr.start.as_mut_ptr(),
                                len: addr.end.as_usize() - addr.start.as_usize(),
                                permissions,
                            },
                        )
                    }
                    CapabilityResource::Mmio(phys, _, interrupts) => {
                        // FIXME: check if this device has already been mapped
                        let virt = unsafe {
                            task_state.memory_manager.map_mmio_device(
                                phys.start,
                                None,
                                phys.end.as_usize() - phys.start.as_usize(),
                            )
                        };

                        let plic = PLIC.lock();
                        let plic = plic.as_ref().unwrap();
                        let tid = task.tid;
                        let n_interrupts = interrupts.len();
                        for interrupt in interrupts.iter().copied() {
                            // FIXME: This is copy/pasted from the `ClaimDevice` syscall, maybe
                            // refactor them both out to a function or something?
                            log::debug!("Reregistering interrupt {} to task {}", interrupt, task.name,);
                            plic.enable_interrupt(crate::platform::current_plic_context(), interrupt);
                            plic.set_context_threshold(crate::platform::current_plic_context(), 0);
                            plic.set_interrupt_priority(interrupt, 7);
                            crate::interrupts::isr::register_isr(interrupt, move |plic, _, id| {
                                plic.disable_interrupt(crate::platform::current_plic_context(), id);
                                let task = TASKS.get(tid).unwrap();
                                let mut task_state = task.mutable_state.lock();

                                log::debug!(
                                    "Interrupt {} triggered (hart: {}), notifying task {}",
                                    id,
                                    HART_ID.get(),
                                    task.name
                                );

                                task_state.claimed_interrupts.insert(id, HART_ID.get());

                                // FIXME: not sure if this is entirely correct..
                                let sender = task_state.kernel_channel.sender.clone();

                                drop(task_state);

                                sender
                                    .send(ChannelMessage {
                                        data: Into::into(KernelMessage::InterruptOccurred(id)),
                                        caps: Vec::new(),
                                    })
                                    .expect("handle kernel channel send failure");

                                Ok(())
                            });
                        }

                        let cptr = task_state.cspace.mint(Capability {
                            resource: CapabilityResource::Mmio(phys.clone(), virt.clone(), interrupts),
                            rights,
                        });

                        (
                            cptr,
                            librust::capabilities::CapabilityDescription::MappedMmio {
                                ptr: virt.start.as_mut_ptr(),
                                len: phys.end.as_usize() - phys.start.as_usize(),
                                n_interrupts,
                            },
                        )
                    }
                };

                *target = librust::capabilities::CapabilityWithDescription {
                    capability: librust::capabilities::Capability { cptr, rights },
                    description,
                };
            }

            (n_caps_to_write, caps.len())
        }
    };

    if caps_remaining != 0 {
        channel.receiver.inner.lock().push_front(ChannelMessage { data: [0; 7], caps });
    }

    regs.a1 = caps_written;
    regs.a2 = caps_remaining;
    regs.t0 = data[0];
    regs.t1 = data[1];
    regs.t2 = data[2];
    regs.t3 = data[3];
    regs.t4 = data[4];
    regs.t5 = data[5];
    regs.t6 = data[6];

    log::debug!("[{}:{}:{:?}] Read channel message! ra={:#p}", task.name, task.tid, cptr, crate::asm::ra());
    Ok(())
}
