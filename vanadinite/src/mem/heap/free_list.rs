pub struct LinkedListAllocator {
    origin: *mut LinkedListNode,
    //last_mapped_address:
}

#[derive(Clone, Copy)]
#[repr(C)]
struct LinkedListNode {
    next: Option<core::ptr::NonNull<LinkedListNode>>,
}

impl LinkedListNode {
    fn data(&self) -> *const u8 {
        unsafe { (self as *const _ as *const u8).add(core::mem::size_of::<Self>()) }
    }
}
