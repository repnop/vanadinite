// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::{AtomicBool, Ordering};

#[macro_export]
macro_rules! thread_local {
    ($($v:vis static $name:ident: $ty:ty = $val:expr;)+) => {
        $(
            #[link_section = ".kernel_thread_local"]
            #[thread_local]
            // FIXME: temporarily assert that alignment is 8 or lower, until we have a better heap allocator
            $v static $name: crate::thread_local::ThreadLocal<$ty> = unsafe { const _: () = [()][!(core::mem::align_of::<$ty>() <= 8) as usize]; crate::thread_local::ThreadLocal::new(|| $val) };
        )+
    };
}

pub struct ThreadLocal<T: Send + 'static>(AtomicBool, core::cell::UnsafeCell<core::mem::MaybeUninit<T>>, fn() -> T);

impl<T: Send + 'static> ThreadLocal<T> {
    #[doc(hidden)]
    pub const unsafe fn new(init: fn() -> T) -> Self {
        Self(AtomicBool::new(false), core::cell::UnsafeCell::new(core::mem::MaybeUninit::uninit()), init)
    }

    //pub fn get(&'static self) -> &'static T {
    //    self.init_if_needed()
    //}

    pub fn with<R, F: FnOnce(&T) -> R>(&'static self, f: F) -> R {
        f(self.init_if_needed())
    }

    fn init_if_needed(&self) -> &T {
        let state = self.0.load(Ordering::Relaxed);

        match state {
            true => unsafe { (&*self.1.get()).assume_init_ref() },
            false => {
                assert!(!self.0.compare_and_swap(false, true, Ordering::AcqRel));
                unsafe { self.1.get().write(core::mem::MaybeUninit::new((self.2)())) };
                unsafe { (&*self.1.get()).assume_init_ref() }
            }
            _ => unreachable!(),
        }
    }
}

impl<T: Send + 'static> core::ops::Deref for ThreadLocal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.init_if_needed()
    }
}

pub unsafe fn init_thread_locals() {
    use crate::utils::LinkerSymbol;

    extern "C" {
        static __kernel_thread_local_start: LinkerSymbol;
        static __kernel_thread_local_end: LinkerSymbol;
    }

    let size = __kernel_thread_local_end.as_usize() - __kernel_thread_local_start.as_usize();

    let original_thread_locals = core::slice::from_raw_parts(__kernel_thread_local_start.as_ptr(), size);
    let new_thread_locals = alloc::alloc::alloc_zeroed(alloc::alloc::Layout::from_size_align(size, 8).unwrap());

    core::slice::from_raw_parts_mut(new_thread_locals, size)[..].copy_from_slice(original_thread_locals);

    asm!("mv tp, {}", in(reg) new_thread_locals);
}

pub fn tp() -> *mut u8 {
    let val: usize;
    unsafe { asm!("mv {}, tp", out(reg) val) };

    val as *mut u8
}
