// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::num::NonZeroUsize;

use crate::{
    capabilities::{Capability, CapabilityResource, CapabilitySpace},
    mem::{
        manager::{AddressRegionKind, FillOption, MemoryManager, RegionDescription},
        paging::{flags::Flags, PageSize, VirtualAddress},
    },
    platform::FDT,
    syscall::{channel::UserspaceChannel, vmspace::VmspaceObject},
    trap::{FloatingPointRegisters, GeneralRegisters},
    utils::{round_up_to_next, Units},
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use fdt::Fdt;
use librust::{
    capabilities::CapabilityRights,
    syscalls::{channel::KERNEL_CHANNEL, vmspace::VmspaceObjectId},
    task::Tid,
};

#[derive(Debug)]
#[repr(C)]
pub struct ThreadControlBlock {
    pub kernel_stack: *mut u8,
    pub kernel_thread_local: *mut u8,
    pub kernel_global_ptr: *mut u8,
    pub saved_sp: usize,
    pub saved_tp: usize,
    pub saved_gp: usize,
    pub kernel_stack_size: usize,
}

impl ThreadControlBlock {
    pub fn new() -> Self {
        Self {
            kernel_stack: core::ptr::null_mut(),
            kernel_thread_local: core::ptr::null_mut(),
            kernel_global_ptr: core::ptr::null_mut(),
            saved_sp: 0,
            saved_tp: 0,
            saved_gp: 0,
            kernel_stack_size: 0,
        }
    }

    /// # Safety
    /// This assumes that the pointer to the [`ThreadControlBlock`] has been set
    /// in the `sstatus` register
    pub unsafe fn the() -> *mut Self {
        let ret;
        core::arch::asm!("csrr {}, sstatus", out(reg) ret);
        ret
    }
}

unsafe impl Send for ThreadControlBlock {}
unsafe impl Sync for ThreadControlBlock {}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Context {
    pub gp_regs: GeneralRegisters,
    pub fp_regs: FloatingPointRegisters,
    pub pc: usize,
}

pub struct Task {
    pub tid: Tid,
    pub name: Box<str>,
    pub context: Context,
    pub memory_manager: MemoryManager,
    pub state: TaskState,
    pub vmspace_objects: BTreeMap<VmspaceObjectId, VmspaceObject>,
    pub vmspace_next_id: usize,
    pub cspace: CapabilitySpace,
    pub kernel_channel: UserspaceChannel,
    pub claimed_interrupts: BTreeMap<usize, usize>,
    pub subscribes_to_events: bool,
}

impl Task {
    pub fn load_init<'a>(bin: &[u8], args: impl Iterator<Item = &'a str> + Clone) -> Self {
        let mut memory_manager = MemoryManager::new();
        let mut cspace = CapabilitySpace::new();

        memory_manager.alloc_region(
            Some(VirtualAddress::new(0xF00D_0000)),
            RegionDescription {
                len: round_up_to_next(bin.len(), 4.kib()) / 4.kib(),
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
                len: 32,
                contiguous: false,
                flags: Flags::USER | Flags::READ | Flags::WRITE | Flags::VALID,
                fill: FillOption::Unitialized,
                kind: AddressRegionKind::Stack,
            })
            .add(32.kib());

        let fdt_ptr = FDT.load(core::sync::atomic::Ordering::Acquire);
        let fdt_loc = {
            let fdt = unsafe { Fdt::from_ptr(fdt_ptr) }.unwrap();
            let slice = unsafe { core::slice::from_raw_parts(fdt_ptr, fdt.total_size()) };
            memory_manager.alloc_region(
                None,
                RegionDescription {
                    size: PageSize::Kilopage,
                    len: round_up_to_next(fdt.total_size(), 4.kib()) / 4.kib(),
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
                    len: round_up_to_next(total_size, 4.kib()) / 4.kib(),
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
                    len: round_up_to_next(n * 16, 4.kib()) / 4.kib(),
                    contiguous: false,
                    flags: Flags::USER | Flags::READ | Flags::VALID,
                    fill: FillOption::Data(&ptr_list),
                    kind: AddressRegionKind::ReadOnly,
                });

                (n, ptrs.as_usize())
            }
        };

        let context = Context {
            pc: 0xF00D_0000,
            gp_regs: GeneralRegisters {
                sp: sp.as_usize(),
                tp: 0,
                a0,
                a1,
                a2: fdt_loc.start.as_usize(),
                ..Default::default()
            },
            fp_regs: FloatingPointRegisters::default(),
        };

        let (kernel_channel, user_read) = UserspaceChannel::new();
        cspace
            .mint_with_id(
                KERNEL_CHANNEL,
                Capability { resource: CapabilityResource::Channel(user_read), rights: CapabilityRights::READ },
            )
            .expect("[BUG] kernel channel cap already created?");

        Self {
            tid: Tid::new(NonZeroUsize::new(1).unwrap()),
            name: Box::from("init"),
            context,
            memory_manager,
            state: TaskState::Running,
            vmspace_objects: BTreeMap::new(),
            vmspace_next_id: 0,
            cspace,
            kernel_channel,
            claimed_interrupts: BTreeMap::new(),
            subscribes_to_events: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskState {
    Blocked,
    Dead,
    Running,
}

impl TaskState {
    pub fn is_dead(self) -> bool {
        matches!(self, TaskState::Dead)
    }
}
