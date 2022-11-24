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
    scheduler::{Scheduler, WakeToken, SCHEDULER, TASKS},
    sync::{SpinMutex, SpinRwLock},
    task::Task,
    trap::GeneralRegisters,
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
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let wake = Arc::new(SpinMutex::new(None));

            let sender = Sender {
                inner: Arc::clone(&message_queue),
                alive: Arc::clone(&alive),
                wake: Arc::clone(&wake),
                other_tid: None,
                other_cptr: CapabilityPtr::new(usize::MAX),
            };
            let receiver = Receiver { inner: message_queue, alive, wake };

            (sender, receiver)
        };

        let (sender2, receiver2) = {
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let wake = Arc::new(SpinMutex::new(None));

            let sender = Sender {
                inner: Arc::clone(&message_queue),
                alive: Arc::clone(&alive),
                wake: Arc::clone(&wake),
                other_tid: None,
                other_cptr: CapabilityPtr::new(usize::MAX),
            };
            let receiver = Receiver { inner: message_queue, alive, wake };

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
    pub(super) inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) wake: Arc<SpinMutex<Option<WakeToken>>>,
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
    pub(super) inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) wake: Arc<SpinMutex<Option<WakeToken>>>,
    pub(super) other_tid: Option<Tid>,
    pub(super) other_cptr: CapabilityPtr,
}

impl Sender {
    fn try_send(&self, message: ChannelMessage) -> Result<(), ChannelMessage> {
        if !self.alive.load(Ordering::Acquire) {
            log::debug!("Channel to {:?}:{:?} is dead", self.other_tid, self.other_cptr);
            return Err(message);
        }

        // FIXME: set a buffer limit at some point
        let mut lock = self.inner.write();

        lock.push_back(message);
        if let Some(token) = self.wake.lock().take() {
            log::debug!("Waking other side of channel [{:?}:{:?}]", self.other_tid, self.other_cptr);
            SCHEDULER.unblock(token);
        }

        if let Some(task) = self.other_tid.and_then(|tid| TASKS.get(tid)) {
            let task = task.lock();
            if task.subscribes_to_events {
                log::debug!("Enqueuing kernel message for other cptr [{}:{:?}]", task.name, self.other_cptr);
                task.kernel_channel.sender.try_send(ChannelMessage {
                    data: KernelMessage::into_parts(KernelMessage::NewChannelMessage(self.other_cptr)),
                    caps: Vec::new(),
                })?;
            }
        }

        Ok(())
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        // FIXME: this currently breaks sending messages
        // self.alive.store(false, Ordering::Release);
    }
}

pub fn send_message(task: &mut Task, frame: &mut GeneralRegisters) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);
    let caps =
        RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(frame.a2), frame.a3);
    let data = [frame.t0, frame.t1, frame.t2, frame.t3, frame.t4, frame.t5, frame.t6];

    let channel = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::WRITE =>
        {
            channel
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // Fixup caps here so we can error on any invalid caps/slice and not dealloc
    // the message region
    let caps = match caps.len() {
        0 => Vec::new(),
        _ => {
            let cap_slice = match unsafe { caps.validate(&task.memory_manager) } {
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
                match task.cspace.resolve(cptr) {
                    Some(cap) if cap.rights.is_superset(rights) && cap.rights & CapabilityRights::GRANT => {
                        // Can't allow sending invalid memory permissions
                        if let CapabilityResource::Memory(..) = &cap.resource {
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
    // FIXME: this should notify the sender the channel is dead if it is
    channel.sender.try_send(ChannelMessage { data, caps }).unwrap();

    Ok(())
}

pub fn read_message(task: &mut Task, regs: &mut GeneralRegisters) -> Result<super::Outcome, SyscallError> {
    let cptr = CapabilityPtr::new(regs.a1);
    let cap_buffer = RawUserSlice::<user::ReadWrite, librust::capabilities::CapabilityWithDescription>::new(
        VirtualAddress::new(regs.a2),
        regs.a3,
    );
    let flags = ChannelReadFlags::new(regs.a4);

    let channel = match task.cspace.resolve(cptr) {
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

    let mut receiver = channel.receiver.inner.write();
    let mut wake_lock = channel.receiver.wake.lock();
    match receiver.pop_front() {
        None if flags & ChannelReadFlags::NONBLOCKING => Err(SyscallError::WouldBlock),
        None => {
            log::debug!("[{}:{}:{:?}] Registering wake for channel::read_message", task.name, task.tid, cptr);
            wake_lock.replace(WakeToken::new(task.tid, move |task| {
                log::debug!("Waking task {:?} (TID: {:?}) for channel::read_message!", task.name, task.tid.value());
                let mut regs = task.context.gp_regs;
                let cptr = CapabilityPtr::new(regs.a1);
                let res = read_message(task, &mut regs);
                match res {
                    Ok(super::Outcome::Completed) => {
                        regs.a0 = 0;
                        task.context.gp_regs = regs;
                        log::debug!("[{}:{}:{:?}] Completed blocked read", task.name, task.tid, cptr);
                    }
                    _ => todo!("is this even possible?"),
                }
            }));

            Ok(super::Outcome::Blocked)
        }
        Some(ChannelMessage { data, mut caps }) => {
            let (caps_written, caps_remaining) = match cap_buffer.len() {
                0 => (0, caps.len()),
                len => {
                    let cap_slice = match unsafe { cap_buffer.validate(&task.memory_manager) } {
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

                                let mut other_task = other_task.lock();
                                let cptr = task.cspace.mint_with(|this_cptr| {
                                    other_task.cspace.mint_with(|other_cptr| {
                                        c1.sender.other_cptr = this_cptr;
                                        c2.sender.other_cptr = other_cptr;
                                        Capability { resource: CapabilityResource::Channel(c1), rights }
                                    });

                                    Capability { resource: CapabilityResource::Channel(c2), rights }
                                });

                                (cptr, librust::capabilities::CapabilityDescription::Channel)
                            }
                            CapabilityResource::Memory(region, _, kind) => {
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

                                let addr =
                                    task.memory_manager.apply_shared_region(None, memflags, region.clone(), kind);

                                let cptr = task.cspace.mint(Capability {
                                    resource: CapabilityResource::Memory(region, addr.clone(), kind),
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
                                    task.memory_manager.map_mmio_device(
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
                                        let mut task = task.lock();

                                        log::debug!(
                                            "Interrupt {} triggered (hart: {}), notifying task {}",
                                            id,
                                            HART_ID.get(),
                                            task.name
                                        );

                                        task.claimed_interrupts.insert(id, HART_ID.get());

                                        // FIXME: not sure if this is entirely correct..
                                        let mut send_lock = task.kernel_channel.sender.inner.write();
                                        send_lock.push_back(ChannelMessage {
                                            data: Into::into(KernelMessage::InterruptOccurred(id)),
                                            caps: Vec::new(),
                                        });

                                        let token = task.kernel_channel.sender.wake.lock().take();

                                        if let Some(token) = token {
                                            drop(send_lock);
                                            drop(task);
                                            SCHEDULER.unblock(token);
                                        }

                                        Ok(())
                                    });
                                }

                                let cptr = task.cspace.mint(Capability {
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
                receiver.push_front(ChannelMessage { data: [0; 7], caps });
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
            Ok(super::Outcome::Completed)
        }
    }
}
