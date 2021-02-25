// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{AtomicBool, Ordering};

#[macro_export]
macro_rules! cpu_local {
    ($($v:vis static $name:ident: $ty:ty = $val:expr;)+) => {
        $(
            #[thread_local]
            // FIXME: temporarily assert that alignment is 8 or lower, until we have a better heap allocator
            $v static $name: crate::cpu_local::CpuLocal<$ty> = unsafe { const _: () = [()][!(core::mem::align_of::<$ty>() <= 8) as usize]; crate::cpu_local::CpuLocal::new(|| $val) };
        )+
    };
}

pub struct CpuLocal<T: Send + 'static>(AtomicBool, core::cell::UnsafeCell<core::mem::MaybeUninit<T>>, fn() -> T);

impl<T: Send + 'static> CpuLocal<T> {
    #[doc(hidden)]
    pub const unsafe fn new(init: fn() -> T) -> Self {
        Self(AtomicBool::new(false), core::cell::UnsafeCell::new(core::mem::MaybeUninit::uninit()), init)
    }

    pub fn with<R, F: FnOnce(&T) -> R>(&'static self, f: F) -> R {
        f(self.init_if_needed())
    }

    // FIXME: use 3 states
    fn init_if_needed(&self) -> &T {
        let state = self.0.load(Ordering::Relaxed);

        match state {
            true => unsafe { (&*self.1.get()).assume_init_ref() },
            false => {
                assert!(self.0.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok());
                unsafe { self.1.get().write(core::mem::MaybeUninit::new((self.2)())) };
                unsafe { (&*self.1.get()).assume_init_ref() }
            }
        }
    }
}

impl<T: Send + 'static> core::ops::Deref for CpuLocal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.init_if_needed()
    }
}

/// # Safety
///
/// This function ***must*** be called before any references to any per-hart
/// statics are used, and failing to do so can result in undefined behavior
pub unsafe fn init_thread_locals() {
    use crate::utils::LinkerSymbol;

    extern "C" {
        static __tdata_start: LinkerSymbol;
        static __tdata_end: LinkerSymbol;
    }

    let size = __tdata_end.as_usize() - __tdata_start.as_usize();

    let original_thread_locals = core::slice::from_raw_parts(__tdata_start.as_ptr(), size);
    let new_thread_locals = alloc::alloc::alloc_zeroed(alloc::alloc::Layout::from_size_align(size, 8).unwrap());

    core::slice::from_raw_parts_mut(new_thread_locals, size)[..].copy_from_slice(original_thread_locals);

    asm!("mv tp, {}", in(reg) new_thread_locals);
}

pub fn tp() -> *mut u8 {
    let val: usize;
    unsafe { asm!("mv {}, tp", out(reg) val) };

    val as *mut u8
}