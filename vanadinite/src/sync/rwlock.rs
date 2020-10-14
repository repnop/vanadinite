// FIXME: Fix up atomic ordering

use core::sync::atomic::{spin_loop_hint, AtomicU32, Ordering};

const WRITE_LOCKED: u32 = 1 << 31;

pub struct SpinRwLock {
    lock: AtomicU32,
}

impl SpinRwLock {
    pub const fn new() -> Self {
        Self { lock: AtomicU32::new(0) }
    }
}

unsafe impl lock_api::RawRwLock for SpinRwLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = SpinRwLock::new();

    type GuardMarker = lock_api::GuardSend;

    fn lock_shared(&self) {
        while !self.try_lock_shared() {
            spin_loop_hint();
        }
    }

    fn try_lock_shared(&self) -> bool {
        let value = self.lock.fetch_add(1, Ordering::SeqCst);
        match value & WRITE_LOCKED == WRITE_LOCKED {
            true => {
                self.lock.fetch_sub(1, Ordering::SeqCst);
                false
            }
            false => true,
        }
    }

    unsafe fn unlock_shared(&self) {
        self.lock.fetch_sub(1, Ordering::SeqCst);
    }

    fn lock_exclusive(&self) {
        while !self.try_lock_exclusive() {
            spin_loop_hint();
        }
    }

    fn try_lock_exclusive(&self) -> bool {
        let lock = self.lock.load(Ordering::SeqCst);

        match lock {
            0 => {
                self.lock.store(WRITE_LOCKED, Ordering::SeqCst);
                true
            }
            _ => false,
        }
    }

    unsafe fn unlock_exclusive(&self) {
        self.lock.store(0, Ordering::SeqCst);
    }
}
