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
        Self { inner: Mutex::new(FreeList { origin: None, limit: VirtualAddress::new(0) }) }
    }
}

unsafe impl Send for FreeListAllocator {}
unsafe impl Sync for FreeListAllocator {}

// FIXME: fragmented as heck
unsafe impl alloc::alloc::GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut inner = self.inner.lock();

        log::info!("FreeListAllocator::alloc: allocating {:?}", layout);
        let size = align_to_usize(layout.size());

        if layout.align() > 8 {
            todo!("FreeListAllocator::alloc: >8 byte alignment");
        }

        match inner.origin {
            Some(ptr) => {
                let mut ptr = ptr;

                loop {
                    {
                        let r = ptr.as_mut();
                        log::info!("{:?}", r);

                        // See if we can just keep going
                        if let (Some(next), State::Occupied) = (r.next, r.state) {
                            ptr = next;
                            continue;
                        }

                        // Check to see if this is a free, appropriate slot that we can't split off
                        let enough_for_new_node = r.size - size > core::mem::size_of::<FreeListNode>();
                        if r.state.is_free() && r.size >= size && !enough_for_new_node {
                            r.state = State::Occupied;
                            break r.data();
                        }

                        // If its free and enough, split it and return the ptr to the current
                        if r.next.is_none() && r.state.is_free() && enough_for_new_node {
                            let new_node_ptr = NonNull::new(r.data().add(size).cast()).unwrap();
                            let new_node = FreeListNode {
                                next: None,
                                size: r.size - (size + core::mem::size_of::<FreeListNode>()),
                                state: State::Free,
                            };

                            *new_node_ptr.as_ptr() = new_node;
                            r.size = size;
                            r.state = State::Occupied;
                            r.next = Some(new_node_ptr);
                            break r.data();
                        }

                        // Otherwise, looks like we need to allocate more memory
                        // Should be last node?
                        // assert!(
                        //     r.next.is_none(),
                        //     "FreeListAllocator::alloc: hit a node that wasn't last but had to alloc more? {:?}",
                        //     r
                        // );

                        if let Some(next) = r.next {
                            ptr = next;
                            continue;
                        }
                    }

                    let (mut p, _) = inner.alloc_more(size);

                    let r = p.as_mut();
                    let new_node_ptr = NonNull::new(r.data().add(size).cast()).unwrap();
                    let new_node = FreeListNode {
                        next: None,
                        size: r.size - (size + core::mem::size_of::<FreeListNode>()),
                        state: State::Free,
                    };

                    *new_node_ptr.as_ptr() = new_node;
                    r.size = size;
                    r.state = State::Occupied;
                    r.next = Some(new_node_ptr);
                    break r.data();
                }
            }
            None => {
                inner.alloc_more(size);

                inner.origin.unwrap().as_ref().data()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _: core::alloc::Layout) {
        assert!(!ptr.is_null());

        let _inner = self.inner.lock();
        let ptr = (ptr as usize - core::mem::size_of::<FreeListNode>()) as *mut FreeListNode;
        (&mut *ptr).state = State::Free;
    }
}

struct FreeList {
    origin: Option<core::ptr::NonNull<FreeListNode>>,
    limit: VirtualAddress,
}

impl FreeList {
    unsafe fn alloc_more(&mut self, size: usize) -> (NonNull<FreeListNode>, usize) {
        let size_with_node = size + core::mem::size_of::<FreeListNode>();
        let num_pages = size_with_node / KIB_PAGE_SIZE + 1;

        let new_mem_size = num_pages * KIB_PAGE_SIZE;
        let start = match self.limit.as_usize() {
            0 => VirtualAddress::new(super::HEAP_START),
            _ => self.limit,
        };
        let end = VirtualAddress::new(start.as_usize() + new_mem_size);

        if num_pages == 1 {
            PAGE_TABLE_MANAGER.lock().alloc_virtual(start, Read | Write);
            self.limit = end;
        } else {
            PAGE_TABLE_MANAGER.lock().alloc_virtual_range(start, new_mem_size, Read | Write);
            self.limit = VirtualAddress::new(end.as_usize() + KIB_PAGE_SIZE);
        }

        let free_node = match self.origin {
            Some(mut ptr) => {
                while let Some(next) = ptr.as_ref().next {
                    ptr = next;
                }

                match ptr.as_ref().state {
                    State::Free => ptr.as_mut().size += new_mem_size,
                    State::Occupied => {
                        let bytes_to_next = ptr.as_ref().size + core::mem::size_of::<FreeListNode>();
                        ptr = NonNull::new(ptr.as_ptr().cast::<u8>().add(bytes_to_next).cast()).unwrap();

                        *ptr.as_ptr() = FreeListNode { next: None, size: new_mem_size, state: State::Free };
                    }
                }

                ptr
            }
            None => {
                let ptr = NonNull::new(super::HEAP_START as *mut FreeListNode).unwrap();
                let next_node_ptr = NonNull::new((super::HEAP_START as *mut u8).add(size_with_node).cast()).unwrap();

                let first_list_node = FreeListNode { next: Some(next_node_ptr), size, state: State::Occupied };

                let second_list_node =
                    FreeListNode { next: None, size: new_mem_size - size_with_node, state: State::Free };

                *ptr.as_ptr() = first_list_node;
                *next_node_ptr.as_ptr() = second_list_node;

                self.origin = Some(ptr);

                ptr
            }
        };

        (free_node, new_mem_size)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
enum State {
    Free,
    Occupied,
}

impl State {
    fn is_free(self) -> bool {
        match self {
            State::Free => true,
            State::Occupied => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FreeListNode {
    next: Option<core::ptr::NonNull<FreeListNode>>,
    size: usize,
    state: State,
}

impl FreeListNode {
    fn data(&self) -> *mut u8 {
        unsafe { (self as *const _ as *const u8 as *mut u8).add(core::mem::size_of::<Self>()) }
    }
}

fn align_to_usize(n: usize) -> usize {
    (n + core::mem::size_of::<usize>() - 1) & !(core::mem::size_of::<usize>() - 1)
}
