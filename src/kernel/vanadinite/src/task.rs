// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{cell::Cell, num::NonZeroUsize};

use crate::{
    capabilities::{Capability, CapabilityResource, CapabilitySpace},
    mem::{
        alloc_kernel_stack,
        manager::{AddressRegionKind, FillOption, RegionDescription, UserspaceMemoryManager},
        paging::{flags::Flags, PageSize, VirtualAddress},
    },
    platform::FDT,
    sync::SpinMutex,
    syscall::{channel::UserspaceChannel, vmspace::VmspaceObject},
    trap::{GeneralRegisters, TrapFrame},
    utils::{round_up_to_next, SameHartDeadlockDetection, Units},
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use fdt::Fdt;
use librust::{
    capabilities::CapabilityRights,
    syscalls::{channel::KERNEL_CHANNEL, vmspace::VmspaceObjectId},
    task::Tid,
};

#[thread_local]
pub static HART_SSCRATCH: Cell<Sscratch> = Cell::new(Sscratch::new());

#[derive(Debug)]
#[repr(C)]
pub struct Sscratch {
    pub kernel_stack_top: *mut u8,
    pub kernel_thread_local: *mut u8,
    pub kernel_global_ptr: *mut u8,
    pub scratch_sp: usize,
}

impl Sscratch {
    pub const fn new() -> Self {
        Self {
            kernel_stack_top: core::ptr::null_mut(),
            kernel_thread_local: core::ptr::null_mut(),
            kernel_global_ptr: core::ptr::null_mut(),
            scratch_sp: 0,
        }
    }
}

unsafe impl Send for Sscratch {}
unsafe impl Sync for Sscratch {}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Context {
    pub ra: usize,
    pub sp: usize,
    pub sx: [usize; 12],
}

pub struct MutableState {
    pub memory_manager: UserspaceMemoryManager,
    pub vmspace_objects: BTreeMap<VmspaceObjectId, VmspaceObject>,
    pub vmspace_next_id: usize,
    pub cspace: CapabilitySpace,
    pub kernel_channel: UserspaceChannel,
    pub claimed_interrupts: BTreeMap<usize, usize>,
    pub subscribes_to_events: bool,
}

#[derive(Debug)]
pub struct Task {
    pub tid: Tid,
    pub name: Box<str>,
    pub kernel_stack: *mut u8,
    pub context: SpinMutex<Context>,
    pub mutable_state: SpinMutex<MutableState, SameHartDeadlockDetection>,
}

impl Task {
    pub fn load_init<'a>(bin: &[u8], args: impl Iterator<Item = &'a str> + Clone) -> Self {
        let mut memory_manager = UserspaceMemoryManager::new();
        let mut cspace = CapabilitySpace::new();

        memory_manager.alloc_region(
            Some(VirtualAddress::new(0xF00D_0000)),
            RegionDescription {
                count: round_up_to_next(bin.len(), 4.kib()) / 4.kib(),
                size: PageSize::Kilopage,
                contiguous: false,
                flags: Flags::USER | Flags::READ | Flags::WRITE | Flags::EXECUTE | Flags::VALID,
                fill: FillOption::Data(bin),
                kind: AddressRegionKind::Text,
            },
        );

        let sp = memory_manager
            .alloc_guarded_region(RegionDescription {
                size: PageSize::Kilopage,
                count: 128,
                contiguous: false,
                flags: Flags::USER | Flags::READ | Flags::WRITE | Flags::VALID,
                fill: FillOption::Unitialized,
                kind: AddressRegionKind::Stack,
            })
            .add(128.kib());

        let fdt_ptr = FDT.load(core::sync::atomic::Ordering::Acquire);
        let fdt_loc = {
            let fdt = unsafe { Fdt::from_ptr(fdt_ptr) }.unwrap();
            let slice = unsafe { core::slice::from_raw_parts(fdt_ptr, fdt.total_size()) };
            memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: PageSize::Kilopage,
                    count: round_up_to_next(fdt.total_size(), 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: Flags::USER | Flags::READ | Flags::VALID,
                    fill: FillOption::Data(slice),
                    kind: AddressRegionKind::Data,
                },
            )
        };

        let arg_count = args.clone().count();
        let (a0, a1) = match arg_count {
            0 => (0, 0),
            n => {
                let total_size = args.clone().fold(0, |total, s| total + s.len());
                let concatenated = args.clone().flat_map(|s| s.bytes()).collect::<Vec<_>>();
                let storage = memory_manager.alloc_guarded_region(RegionDescription {
                    size: PageSize::Kilopage,
                    count: round_up_to_next(total_size, 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: Flags::USER | Flags::READ | Flags::VALID,
                    fill: FillOption::Data(&concatenated),
                    kind: AddressRegionKind::ReadOnly,
                });
                let (_, ptr_list) = args.fold((storage, Vec::new()), |(ptr, mut v), s| {
                    v.extend_from_slice(&ptr.as_usize().to_ne_bytes());
                    v.extend_from_slice(&s.len().to_ne_bytes());

                    (ptr.add(s.len()), v)
                });
                let ptrs = memory_manager.alloc_guarded_region(RegionDescription {
                    size: PageSize::Kilopage,
                    count: round_up_to_next(n * 16, 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: Flags::USER | Flags::READ | Flags::VALID,
                    fill: FillOption::Data(&ptr_list),
                    kind: AddressRegionKind::ReadOnly,
                });

                (n, ptrs.as_usize())
            }
        };

        let (kernel_channel, user_read) = UserspaceChannel::new();
        cspace
            .mint_with_id(
                KERNEL_CHANNEL,
                Capability { resource: CapabilityResource::Channel(user_read), rights: CapabilityRights::READ },
            )
            .expect("[BUG] kernel channel cap already created?");

        let kernel_stack = alloc_kernel_stack(2.mib());
        let trap_frame = unsafe { kernel_stack.sub(core::mem::size_of::<TrapFrame>()).cast::<TrapFrame>() };
        unsafe {
            *trap_frame = TrapFrame {
                sepc: 0xF00D_0000,
                registers: GeneralRegisters {
                    sp: sp.as_usize(),
                    a0,
                    a1,
                    a2: fdt_loc.start.as_usize(),
                    ..Default::default()
                },
            }
        };

        Self {
            tid: Tid::new(NonZeroUsize::new(1).unwrap()),
            name: Box::from("init"),
            context: SpinMutex::new(Context {
                ra: crate::scheduler::return_to_usermode as usize,
                sp: kernel_stack.addr() - core::mem::size_of::<TrapFrame>(),
                sx: [0; 12],
            }),
            kernel_stack,
            mutable_state: SpinMutex::new(MutableState {
                memory_manager,
                vmspace_objects: BTreeMap::new(),
                vmspace_next_id: 0,
                cspace,
                kernel_channel,
                claimed_interrupts: BTreeMap::new(),
                subscribes_to_events: false,
            }),
        }
    }

    /// Creates a task which will idle and wait for interrupts in userspace
    pub fn idle() -> Self {
        log::trace!("[Task::idle] Entered");
        let mut memory_manager = UserspaceMemoryManager::new();
        let mut cspace = CapabilitySpace::new();
        log::trace!("[Task::idle] Allocating instructions");
        memory_manager.alloc_region(
            Some(VirtualAddress::new(0xF00D_0000)),
            RegionDescription {
                count: 1,
                size: PageSize::Kilopage,
                contiguous: false,
                flags: Flags::USER | Flags::READ | Flags::WRITE | Flags::EXECUTE | Flags::VALID,
                fill: FillOption::Data(&[
                    0x0f, 0x00, 0x00, 0x01, // wfi
                    0x6f, 0xf0, 0xdf, 0xff, // j -4
                ]),
                kind: AddressRegionKind::Text,
            },
        );

        log::trace!("[Task::idle] Allocating kernel stack");
        let kernel_stack = alloc_kernel_stack(2.mib());
        let trap_frame = unsafe { kernel_stack.sub(core::mem::size_of::<TrapFrame>()).cast::<TrapFrame>() };
        unsafe { *trap_frame = TrapFrame { sepc: 0xF00D_0000, registers: GeneralRegisters { ..Default::default() } } };

        let (kernel_channel, user_read) = UserspaceChannel::new();
        cspace
            .mint_with_id(
                KERNEL_CHANNEL,
                Capability { resource: CapabilityResource::Channel(user_read), rights: CapabilityRights::READ },
            )
            .expect("[BUG] kernel channel cap already created?");

        log::trace!("[Task::idle] Returning idle task");

        Self {
            tid: Tid::new(NonZeroUsize::new(usize::MAX).unwrap()),
            name: Box::from("<idle>"),
            context: SpinMutex::new(Context {
                ra: crate::scheduler::return_to_usermode as usize,
                sp: kernel_stack.addr() - core::mem::size_of::<TrapFrame>(),
                sx: [0; 12],
            }),
            kernel_stack,
            mutable_state: SpinMutex::new(MutableState {
                memory_manager,
                vmspace_objects: BTreeMap::new(),
                vmspace_next_id: 0,
                cspace,
                kernel_channel,
                claimed_interrupts: BTreeMap::new(),
                subscribes_to_events: false,
            }),
        }
    }
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Blocked,
    Dead,
    Ready,
    Running,
}

impl TaskState {
    pub fn is_dead(self) -> bool {
        matches!(self, TaskState::Dead)
    }
}
