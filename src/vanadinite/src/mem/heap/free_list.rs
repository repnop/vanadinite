// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    mem::paging::{Read, VirtualAddress, Write, KIB_PAGE_SIZE, PAGE_TABLE_MANAGER},
    sync::Mutex,
};
use core::ptr::NonNull;

pub struct FreeListAllocator {
    inner: Mutex<FreeList>,
}

impl FreeListAllocator {
    pub const fn new() -> Self {
        Self { inner: Mutex::new(FreeList { origin: None, limit: VirtualAddress::new(super::HEAP_START) }) }
    }
}

unsafe impl Send for FreeListAllocator {}
unsafe impl Sync for FreeListAllocator {}

// FIXME: fragmented as heck
unsafe impl alloc::alloc::GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut this = self.inner.lock();

        log::info!("FreeListAllocator::alloc: allocating {:?}", layout);
        let size = align_to_usize(layout.size());

        if layout.align() > 8 {
            todo!("FreeListAllocator::alloc: >8 byte alignment");
        }

        if this.origin.is_none() {
            log::info!("FreeListAllocator::alloc: initializing origin");
            this.origin = Some(NonNull::new_unchecked(this.alloc_more(size)));
        }

        let head = this.origin.unwrap().as_ptr();

        let mut prev_node: Option<*mut FreeListNode> = None;
        let mut node = head;

        log::info!("FreeListAllocator::alloc: head={:?}", &*head);

        loop {
            log::info!("FreeListAllocator::alloc: checking node, node={:?}", &*node);
            // if the node's size is large enough to fit another header + at
            // least 8 bytes, we can split it
            let enough_for_split = (*node).size >= size + FreeListNode::struct_size() + 8;

            if (*node).size >= size && !enough_for_split {
                log::info!("FreeListAllocator::alloc: reusing node, but its not big enough to split");

                match prev_node {
                    Some(prev_node) => (&mut *prev_node).next = (*node).next,
                    None => this.origin = (*node).next,
                }

                break (&*node).data();
            }

            if (*node).size >= size && enough_for_split {
                log::info!("FreeListAllocator::alloc: reusing node and splitting");

                let new_node_ptr: *mut FreeListNode = (&*node).data().add(size).cast();
                (&mut *new_node_ptr).size = (*node).size - (size + FreeListNode::struct_size());
                (&mut *new_node_ptr).next = (*node).next;
                (&mut *node).size = size;

                log::info!(
                    "FreeListAllocator::alloc: created new node, current node={:?}, new node={:?}",
                    &*node,
                    &*new_node_ptr
                );

                let next = Some(NonNull::new_unchecked(new_node_ptr));

                match prev_node {
                    Some(prev_node) => (&mut *prev_node).next = next,
                    None => this.origin = next,
                }

                break (&*node).data();
            }

            match (*node).next {
                Some(next) => {
                    prev_node = Some(node);
                    node = next.as_ptr();
                }
                None => {
                    let new_node_ptr = this.alloc_more(size);
                    (&mut *node).next = Some(NonNull::new_unchecked(new_node_ptr));

                    prev_node = Some(node);
                    node = new_node_ptr;
                }
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _: core::alloc::Layout) {
        assert!(!ptr.is_null());

        let mut inner = self.inner.lock();
        let ptr = (ptr as usize - core::mem::size_of::<FreeListNode>()) as *mut FreeListNode;
        (&mut *ptr).next = inner.origin;
        inner.origin = Some(NonNull::new_unchecked(ptr));
    }
}

struct FreeList {
    origin: Option<core::ptr::NonNull<FreeListNode>>,
    limit: VirtualAddress,
}

impl FreeList {
    unsafe fn alloc_more(&mut self, size: usize) -> *mut FreeListNode {
        log::info!("FreeListAllocator::alloc_more: additional allocation requested, size={}", size);

        let size_with_node = size + FreeListNode::struct_size();
        let num_pages = size_with_node / KIB_PAGE_SIZE + 1;

        let new_mem_size = num_pages * KIB_PAGE_SIZE;
        let start = self.limit;
        let end = start.offset(new_mem_size);

        log::info!("FreeListAllocator::alloc_more: allocating {} pages", num_pages);

        PAGE_TABLE_MANAGER.lock().alloc_virtual_range(start, new_mem_size, Read | Write);
        self.limit = end;

        let node_ptr: *mut FreeListNode = start.as_mut_ptr().cast();
        (&mut *node_ptr).next = None;
        (&mut *node_ptr).size = new_mem_size - FreeListNode::struct_size();

        node_ptr
    }
}

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
}

fn align_to_usize(n: usize) -> usize {
    (n + core::mem::size_of::<usize>() - 1) & !(core::mem::size_of::<usize>() - 1)
}
