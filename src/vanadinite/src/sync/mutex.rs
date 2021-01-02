// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{AtomicBool, Ordering};

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
            crate::asm::pause();
        }
    }

    fn try_lock(&self) -> bool {
        self.lock.compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire).is_ok()
    }

    unsafe fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}
