mod mutex;
mod rwlock;

pub use lock_api::{self, RawMutex};

#[repr(transparent)]
pub struct RwLock<T>(lock_api::RwLock<rwlock::SpinRwLock, T>);

impl<T> RwLock<T> {
    pub const fn new(value: T) -> Self {
        Self(lock_api::RwLock::const_new(rwlock::SpinRwLock::new(), value))
    }
}

impl<T> core::ops::Deref for RwLock<T> {
    type Target = lock_api::RwLock<rwlock::SpinRwLock, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct Mutex<T>(lock_api::Mutex<mutex::SpinMutex, T>);

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self(lock_api::Mutex::const_new(mutex::SpinMutex::new(), value))
    }
}

impl<T> core::ops::Deref for Mutex<T> {
    type Target = lock_api::Mutex<mutex::SpinMutex, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
