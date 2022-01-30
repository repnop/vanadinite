// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    cell::Cell,
    ptr::{self, NonNull},
};
use librust::{
    message::SyscallResult,
    syscalls::allocation::{self, AllocationOptions, MemoryPermissions},
};
use sync::SpinMutex;

#[derive(Clone, Copy)]
pub struct TaskLocal(core::marker::PhantomData<*mut ()>);

impl TaskLocal {
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

unsafe impl Allocator for TaskLocal {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
        TASK_LOCAL_ALLOCATOR.lock().allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        TASK_LOCAL_ALLOCATOR.lock().deallocate(ptr, layout)
    }
}

// #[thread_local]
static TASK_LOCAL_ALLOCATOR: SpinMutex<TaskLocalAllocator> = SpinMutex::new(TaskLocalAllocator::new());

unsafe impl Send for TaskLocalAllocator {}
struct TaskLocalAllocator {
    slabs: [(usize, Cell<*mut u8>); 16],
    // TODO: have a catch-all backup for allocations >32KiB
}

impl TaskLocalAllocator {
    const fn new() -> Self {
        let slabs = [
            (8, Cell::new(core::ptr::null_mut::<u8>())),
            (16, Cell::new(core::ptr::null_mut::<u8>())),
            (32, Cell::new(core::ptr::null_mut::<u8>())),
            (48, Cell::new(core::ptr::null_mut::<u8>())),
            (64, Cell::new(core::ptr::null_mut::<u8>())),
            (96, Cell::new(core::ptr::null_mut::<u8>())),
            (128, Cell::new(core::ptr::null_mut::<u8>())),
            (192, Cell::new(core::ptr::null_mut::<u8>())),
            (256, Cell::new(core::ptr::null_mut::<u8>())),
            (512, Cell::new(core::ptr::null_mut::<u8>())),
            (1024, Cell::new(core::ptr::null_mut::<u8>())),
            (2048, Cell::new(core::ptr::null_mut::<u8>())),
            (4096, Cell::new(core::ptr::null_mut::<u8>())),
            (8192, Cell::new(core::ptr::null_mut::<u8>())),
            (16384, Cell::new(core::ptr::null_mut::<u8>())),
            (32768, Cell::new(core::ptr::null_mut::<u8>())),
        ];

        Self { slabs }
    }

    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = usize::max(layout.size(), layout.align().min(4096));

        //println!("Alloc request: {:?}", layout);

        let slab = self.slabs.iter().find(|s| s.0 >= size).ok_or(AllocError)?;
        let mut slab_head = slab.1.get();

        if slab_head.is_null() {
            //println!("Adding new memory region for slab of size {}", slab.0);

            let mem_size = slab.0 * 64;
            let perms = MemoryPermissions::READ | MemoryPermissions::WRITE;
            let mut options = AllocationOptions::None;

            if mem_size >= 2 * 1024 * 1024 {
                //println!("Asking for large pages");
                options = options | AllocationOptions::LargePage;
            }

            let new_mem = match allocation::alloc_virtual_memory(mem_size, options, perms) {
                SyscallResult::Ok(new_mem) => new_mem,
                SyscallResult::Err(_) => return Err(AllocError),
            };

            //println!("New mem is at {:#p}", new_mem);

            for i in 0..63 {
                let curr = unsafe { new_mem.add(i * slab.0).cast::<usize>() };
                let next = unsafe { new_mem.add((i + 1) * slab.0) };

                //println!("Setting {:#p} to point to {:#p}", curr, next);

                unsafe { *curr = next as usize };
            }

            unsafe { *new_mem.add(63 * slab.0).cast::<usize>() = 0 };

            slab_head = new_mem;
        }

        let next_ptr = unsafe { *slab_head.cast::<usize>() } as *mut u8;
        slab.1.set(next_ptr);

        Ok(unsafe { NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(slab_head, slab.0)) })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = usize::max(layout.size(), layout.align().min(4096));
        let slab = self.slabs.iter().find(|s| s.0 >= size).expect("Invalid deallocation");

        *ptr.as_ptr().cast::<usize>() = slab.1.get() as usize;
        slab.1.set(ptr.as_ptr())
    }
}

struct GlobalTaskLocalAllocator;

unsafe impl GlobalAlloc for GlobalTaskLocalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match TASK_LOCAL_ALLOCATOR.lock().allocate(layout) {
            Ok(ptr) => ptr.as_ptr() as *mut u8,
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !ptr.is_null() {
            TASK_LOCAL_ALLOCATOR.lock().deallocate(NonNull::new_unchecked(ptr), layout)
        }
    }
}

#[global_allocator]
static TASK_LOCAL: GlobalTaskLocalAllocator = GlobalTaskLocalAllocator;
