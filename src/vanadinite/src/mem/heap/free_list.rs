// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::sync::Mutex;
use core::ptr::NonNull;

pub struct FreeListAllocator {
    inner: Mutex<FreeList>,
}

impl FreeListAllocator {
    pub const fn new() -> Self {
        Self { inner: Mutex::new(FreeList { head: None, origin: core::ptr::null_mut(), size: 0 }) }
    }

    pub unsafe fn init(&self, origin: *mut u8, size: usize) {
        let mut inner = self.inner.lock();
        inner.head = Some(NonNull::new(origin.cast()).expect("bad origin passed"));
        inner.origin = origin;
        inner.size = size;

        *inner.head.unwrap().as_ptr() = FreeListNode { next: None, size: size - FreeListNode::struct_size() };
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

        log::debug!("FreeListAllocator::alloc: head={:?}", &*head);

        loop {
            log::debug!("FreeListAllocator::alloc: checking node, node={:?}", &*node);
            // if the node's size is large enough to fit another header + at
            // least 8 bytes, we can split it
            let enough_for_split = (*node).size >= size + FreeListNode::struct_size() + 8;

            if (*node).size >= size && !enough_for_split {
                log::debug!("FreeListAllocator::alloc: reusing node, but its not big enough to split");

                match prev_node {
                    Some(prev_node) => (&mut *prev_node).next = (*node).next,
                    None => this.origin = (*node).next.expect("valid next").as_ptr().cast(),
                }

                break (&*node).data();
            }

            if (*node).size >= size && enough_for_split {
                log::debug!("FreeListAllocator::alloc: reusing node and splitting");

                let new_node_ptr: *mut FreeListNode = (&*node).data().add(size).cast();
                (&mut *new_node_ptr).size = (*node).size - (size + FreeListNode::struct_size());
                (&mut *new_node_ptr).next = (*node).next;
                (&mut *node).size = size;
                (&mut *node).next = None;

                log::debug!(
                    "FreeListAllocator::alloc: created new node, current node={:?}, new node={:?}",
                    &*node,
                    &*new_node_ptr
                );

                let next = Some(NonNull::new_unchecked(new_node_ptr));

                match prev_node {
                    Some(prev_node) => (&mut *prev_node).next = next,
                    None => {
                        log::debug!("Setting head to {:?}", &*new_node_ptr);
                        this.head = next;
                    }
                }

                break (&*node).data();
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
        (&mut *ptr).next = inner.head;
        inner.head = Some(NonNull::new_unchecked(ptr));
    }
}

struct FreeList {
    head: Option<NonNull<FreeListNode>>,
    origin: *mut u8,
    size: usize,
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
