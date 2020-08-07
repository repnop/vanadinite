mod mutex;
mod rwlock;

pub type RwLock<T> = lock_api::RwLock<rwlock::SpinRwLock, T>;
pub type Mutex<T> = lock_api::Mutex<mutex::SpinMutex, T>;
