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
        paging::{flags, VirtualAddress},
        user::{self, RawUserSlice},
    },
    scheduler::{Scheduler, WakeToken, SCHEDULER, TASKS},
    task::Task,
    HART_ID, trap::GeneralRegisters,
};
use alloc::{
    collections::VecDeque,
    sync::Arc,
    vec::Vec,
};
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use librust::{
    capabilities::{CapabilityPtr, CapabilityRights},
    error::SyscallError, syscalls::channel::ChannelReadFlags,
};
use sync::{SpinMutex, SpinRwLock};

#[derive(Debug, Clone)]
pub struct UserspaceChannel {
    pub(super) sender: Sender,
    pub(super) receiver: Receiver,
}

impl UserspaceChannel {
    pub fn new() -> (Self, Self) {
        let message_id_counter = Arc::new(AtomicUsize::new(1));
        let (sender1, receiver1) = {
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let wake = Arc::new(SpinMutex::new(None));

            let sender =
                Sender { inner: Arc::clone(&message_queue), alive: Arc::clone(&alive), wake: Arc::clone(&wake) };
            let receiver = Receiver { inner: message_queue, alive, wake };

            (sender, receiver)
        };

        let (sender2, receiver2) = {
            let message_queue = Arc::new(SpinRwLock::new(VecDeque::new()));
            let alive = Arc::new(AtomicBool::new(true));
            let wake = Arc::new(SpinMutex::new(None));

            let sender =
                Sender { inner: Arc::clone(&message_queue), alive: Arc::clone(&alive), wake: Arc::clone(&wake) };
            let receiver = Receiver { inner: message_queue, alive, wake };

            (sender, receiver)
        };

        let first = Self {
            sender: sender1,
            receiver: receiver2,
        };
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
struct Receiver {
    // FIXME: Replace these with something like a lockfree ring buffer
    inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    alive: Arc<AtomicBool>,
    wake: Arc<SpinMutex<Option<WakeToken>>>,
}

impl Receiver {
    fn try_receive(&self) -> Result<Option<ChannelMessage>, ()> {
        // TODO: is it worth trying to `.read()` then `.upgrade()` if not empty?
        match self.inner.write().pop_front() {
            Some(message) => Ok(Some(message)),
            None => match self.alive.load(Ordering::Acquire) {
                true => Ok(None),
                false => Err(()),
            },
        }
    }

    fn register_wake(&self, token: WakeToken) {
        self.wake.lock().replace(token);
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone)]
pub struct Sender {
    // FIXME: Replace these with something like a lockfree ring buffer
    pub(super) inner: Arc<SpinRwLock<VecDeque<ChannelMessage>>>,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) wake: Arc<SpinMutex<Option<WakeToken>>>,
}

impl Sender {
    fn try_send(&self, message: ChannelMessage) -> Result<(), ChannelMessage> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(message);
        }

        // FIXME: set a buffer limit at some point
        self.inner.write().push_back(message);

        if let Some(token) = self.wake.lock().take() {
            SCHEDULER.unblock(token);
        }

        Ok(())
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

pub fn send_message(
    task: &mut Task,
    frame: &mut GeneralRegisters,
) -> Result<(), SyscallError> {
    let cptr = CapabilityPtr::new(frame.a1);
    let caps = RawUserSlice::<user::Read, librust::capabilities::Capability>::new(VirtualAddress::new(frame.a2), frame.a3);
    let data = [
        frame.t0,
        frame.t1,
        frame.t2,
        frame.t3,
        frame.t4,
        frame.t5,
        frame.t6,
    ];

    let current_tid = task.tid;
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
                    Some(cap) if cap.rights.is_superset(rights) && cap.rights & CapabilityRights::GRANT => cloned_caps.push(cap.clone()),
                    _ => return Err(SyscallError::InvalidArgument(2)),
                }
            }

            cloned_caps
        }
    };

    // FIXME: get rid of unwrap, maybe eventually block at a certain limit?
    channel.sender.try_send(ChannelMessage { data, caps }).unwrap();

    Ok(())
}

pub fn read_message(
    task: &mut Task,
    regs: &mut GeneralRegisters,
) -> Result<super::Outcome, SyscallError> {
    let cptr = CapabilityPtr::new(regs.a1);
    let cap_buffer = RawUserSlice::<user::ReadWrite, librust::capabilities::Capability>::new(VirtualAddress::new(regs.a2), regs.a3);
    let flags = ChannelReadFlags::new(regs.a4);

    let channel = match task.cspace.resolve(cptr) {
        Some(Capability { resource: CapabilityResource::Channel(channel), rights })
            if *rights & CapabilityRights::READ =>
        {
            channel
        }
        _ => return Err(SyscallError::InvalidArgument(0)),
    };

    // FIXME: this probably needs the lock to make sure a message wasn't sent
    // after the check but before the register

    // FIXME: check for broken channel

    let mut receiver = channel.receiver.inner.write();
    let mut wake_lock = channel.receiver.wake.lock();
    match receiver.pop_front() {
        None if flags & ChannelReadFlags::NONBLOCKING => return Err(SyscallError::WouldBlock),
        None => {
            log::debug!("Registering wake for channel::read_message");
            SCHEDULER.block(task.tid);
            wake_lock.replace(WakeToken::new(task.tid, move |task| {
                log::debug!("Waking task {:?} (TID: {:?}) for channel::read_message!", task.name, task.tid.value());
                let mut regs = task.context.gp_regs;
                let res = read_message(task, &mut regs);
                match res {
                    Ok(super::Outcome::Completed) => {
                        regs.a0 = 0;
                        task.context.gp_regs = regs;
                    },
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
                        let cptr = match cap.resource {
                            CapabilityResource::Channel(channel) => task.cspace.mint(cap),
                            CapabilityResource::Memory(region, _, kind) => {
                                let addr = task.memory_manager.apply_shared_region(None, flags::USER | flags::READ | flags::WRITE, region, kind);
                                task.cspace.mint(Capability { resource: CapabilityResource::Memory(region, addr, kind), rights })
                            }
                            CapabilityResource::Mmio(phys, _, interrupts) => {
                                // FIXME: check if this device has already been mapped
                                let virt = unsafe { task.memory_manager.map_mmio_device(phys.start, None, phys.end.as_usize() - phys.start.as_usize()) };

                                let plic = PLIC.lock();
                                let plic = plic.as_ref().unwrap();
                                let tid = task.tid;
                                for interrupt in interrupts {
                                    // FIXME: This is copy/pasted from the `ClaimDevice` syscall, maybe
                                    // refactor them both out to a function or something?
                                    log::debug!(
                                        "Reregistering interrupt {} to task {}",
                                        interrupt,
                                        task.name,
                                    );
                                    plic.enable_interrupt(crate::platform::current_plic_context(), interrupt);
                                    plic.set_context_threshold(crate::platform::current_plic_context(), 0);
                                    plic.set_interrupt_priority(interrupt, 7);
                                    crate::interrupts::isr::register_isr(interrupt, move |plic, _, id| {
                                        plic.disable_interrupt(crate::platform::current_plic_context(), id);
                                        let task = TASKS.get(tid).unwrap();
                                        let mut task = task.lock();

                                        log::debug!("Interrupt {} triggered (hart: {}), notifying task {}", id, HART_ID.get(), task.name);

                                        task.claimed_interrupts.insert(id, HART_ID.get());

                                        // FIXME: not sure if this is entirely correct..
                                        let send_lock = task.kernel_channel.sender.inner.write();
                                        send_lock.push_back(ChannelMessage { data: Into::into(KernelMessage::InterruptOccurred(id)), caps: Vec::new() });
                                        if let Some(token) = task.kernel_channel.sender.wake.lock().take() {
                                            drop(task);
                                            drop(send_lock);
                                            SCHEDULER.unblock(token);
                                        }

                                        Ok(())
                                    });
                                }

                                task.cspace.mint(Capability { resource: CapabilityResource::Mmio(phys, virt, interrupts), rights })
                            }
                        };

                        *target = librust::capabilities::Capability { cptr, rights };
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

            Ok(super::Outcome::Completed)
        }
    }
}
