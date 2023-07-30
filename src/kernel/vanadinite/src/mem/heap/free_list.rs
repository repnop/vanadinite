// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::sync::SpinMutex;
use crate::utils::SameHartDeadlockDetection;
use crate::{
    mem::{
        paging::PageSize,
        phys::{PhysicalMemoryAllocator, PHYSICAL_MEMORY_ALLOCATOR},
        phys2virt,
    },
    utils::{round_up_to_next, Units},
};
use core::ptr::NonNull;

pub struct FreeListAllocator {
    inner: SpinMutex<FreeList, SameHartDeadlockDetection>,
}

impl FreeListAllocator {
    pub const fn new() -> Self {
        Self { inner: SpinMutex::new(FreeList { head: None }, SameHartDeadlockDetection::new()) }
    }

    /// Returns the start and end for logging purposes
    pub fn init(&self, size: usize) -> (*mut u8, *mut u8) {
        let n_pages = round_up_to_next(size, 4.kib()) / 4.kib();
        let origin = unsafe {
            phys2virt(
                PHYSICAL_MEMORY_ALLOCATOR
                    .lock()
                    .alloc_contiguous(PageSize::Kilopage, n_pages)
                    .expect("unable to allocate memory for heap")
                    .as_phys_address(),
            )
            .as_mut_ptr()
        };

        let mut inner = self.inner.lock();
        inner.head = Some(NonNull::new(origin.cast()).expect("bad origin passed"));

        unsafe {
            *inner.head.unwrap().as_ptr() = FreeListNode { next: None, size: size - FreeListNode::struct_size() }
        };

        (origin, unsafe { origin.add(round_up_to_next(size, 4.kib())) })
    }
}

unsafe impl Send for FreeListAllocator {}
unsafe impl Sync for FreeListAllocator {}

// FIXME: fragmented as heck
unsafe impl alloc::alloc::GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut this = self.inner.lock();

        log::debug!("FreeListAllocator::alloc: allocating {:?}", layout);
        let size = align_to_usize(layout.size());

        if layout.align() > 8 {
            todo!("FreeListAllocator::alloc: >8 byte alignment");
        }

        let head = this.head.expect("Heap allocator wasn't initialized!").as_ptr();

        let mut prev_node: Option<*mut FreeListNode> = None;
        let mut node = head;

        log::trace!("FreeListAllocator::alloc: head={:?}", &*head);

        loop {
            log::trace!("FreeListAllocator::alloc: checking node, node={:?}", &*node);
            // if the node's size is large enough to fit another header + at
            // least 8 bytes, we can split it
            let enough_for_split = (*node).size >= size + FreeListNode::struct_size() + 8;

            if (*node).size >= size && !enough_for_split {
                log::trace!("FreeListAllocator::alloc: reusing node, but its not big enough to split");

                match prev_node {
                    Some(prev_node) => (*prev_node).next = (*node).next,
                    None => this.head = Some((*node).next.expect("valid next")),
                }

                break (*node).data();
            }

            if (*node).size >= size && enough_for_split {
                log::trace!("FreeListAllocator::alloc: reusing node and splitting");

                let new_node = (*node).split(size);

                log::trace!(
                    "FreeListAllocator::alloc: created new node, current node={:?}, new node={:?}",
                    &*node,
                    &*new_node.as_ptr()
                );

                match prev_node {
                    Some(prev_node) => (*prev_node).next = Some(new_node),
                    None => {
                        log::trace!("Setting head to {:?}", &*new_node.as_ptr());
                        this.head = Some(new_node);
                    }
                }

                break (*node).data();
            }

            match (*node).next {
                Some(next) => {
                    prev_node = Some(node);
                    node = next.as_ptr();
                }
                None => return core::ptr::null_mut(),
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _: core::alloc::Layout) {
        assert!(!ptr.is_null());

        let mut inner = self.inner.lock();
        let ptr = (ptr as usize - core::mem::size_of::<FreeListNode>()) as *mut FreeListNode;

        log::debug!("Freeing {:?}, head={:?}", &*ptr, &*inner.head.unwrap().as_ptr());
        (*ptr).next = inner.head;
        inner.head = Some(NonNull::new_unchecked(ptr));
    }
}

struct FreeList {
    head: Option<NonNull<FreeListNode>>,
}

unsafe impl Send for FreeList {}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FreeListNode {
    next: Option<core::ptr::NonNull<FreeListNode>>,
    size: usize,
}

impl FreeListNode {
    fn data(&self) -> *mut u8 {
        unsafe { (self as *const _ as *const u8 as *mut u8).add(core::mem::size_of::<Self>()) }
    }

    fn struct_size() -> usize {
        core::mem::size_of::<Self>()
    }

    fn split(&mut self, mut new_size: usize) -> NonNull<FreeListNode> {
        assert!(self.size > (new_size + Self::struct_size()), "trying to split off more than is available");

        new_size = align_to_usize(new_size);

        let other_size = self.size - new_size - Self::struct_size();
        self.size = new_size;

        let next_node: *mut Self = unsafe { (self as *mut _ as *mut u8).add(Self::struct_size() + self.size).cast() };
        unsafe { *next_node = FreeListNode { next: self.next.take(), size: other_size } };

        NonNull::new(next_node).unwrap()
    }
}

fn align_to_usize(n: usize) -> usize {
    (n + core::mem::size_of::<usize>() - 1) & !(core::mem::size_of::<usize>() - 1)
}
