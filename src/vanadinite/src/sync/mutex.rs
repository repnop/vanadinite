use core::sync::atomic::{spin_loop_hint, AtomicBool, Ordering};

pub struct SpinMutex {
    lock: AtomicBool,
}

impl SpinMutex {
    pub const fn new() -> Self {
        Self { lock: AtomicBool::new(false) }
    }
}

unsafe impl lock_api::RawMutex for SpinMutex {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = SpinMutex::new();

    type GuardMarker = lock_api::GuardSend;

    fn lock(&self) {
        while !self.try_lock() {
            spin_loop_hint();
        }
    }

    fn try_lock(&self) -> bool {
        self.lock.compare_and_swap(false, true, Ordering::Acquire)
    }

    unsafe fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}
