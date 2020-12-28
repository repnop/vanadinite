use crate::{mem::paging::Sv39PageTable, thread_local, trap::TrapFrame, utils::StaticMut};
use alloc::boxed::Box;
use core::cell::RefCell;

thread_local! {
    pub static THREAD_CONTROL_BLOCK: StaticMut<ThreadControlBlock> = StaticMut::new(ThreadControlBlock::new());
}

#[derive(Debug)]
#[repr(C)]
pub struct ThreadControlBlock {
    pub kernel_stack: *mut u8,
    pub kernel_thread_local: *mut u8,
    pub scratch_space: [usize; 8],
    pub kernel_stack_size: usize,
    pub current_process: Option<Process>,
}

impl ThreadControlBlock {
    pub fn new() -> Self {
        Self {
            kernel_stack: core::ptr::null_mut(),
            kernel_thread_local: core::ptr::null_mut(),
            scratch_space: [0; 8],
            kernel_stack_size: 0,
            current_process: None,
        }
    }
}

unsafe impl Send for ThreadControlBlock {}
unsafe impl Sync for ThreadControlBlock {}

#[derive(Debug)]
pub struct Process {
    pub pid: usize,
    pub pc: *mut u8,
    pub page_table: Box<Sv39PageTable>,
    pub frame: TrapFrame,
    pub state: ProcessState,
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
    Dead,
}