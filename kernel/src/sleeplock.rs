use super::spinlock::SpinMutex;

fn sleep() {
    todo!()
}

pub struct SleepLock {
    locked: SpinMutex<bool>,

    // for debug
    name: &'static str,
    pid: u32,
}

impl SleepLock {
    pub const fn new(name: &'static str) -> Self {
        Self {
            locked: SpinMutex::new("sleep lock", false),
            name,
            pid: u32::MAX,
        }
    }
    pub fn acquire(&self) {
        loop {
            let to_sleep = {
                let mut guard = self.locked.lock();
                if *guard {
                    true
                } else {
                    *guard = true;
                    false
                }
            };
            if to_sleep {
                sleep();
            } else {
                break;
            }
        }
        // self.pid = todo!();
    }
    pub fn release(&self) {
        let mut guard = self.locked.lock();
        *guard = false;
    }
}

use core::cell::UnsafeCell;
pub struct SleepMutex<T> {
    lock: SleepLock,
    data: UnsafeCell<T>,
}
impl<T> SleepMutex<T> {
    pub const fn new(name: &'static str, data: T) -> Self {
        Self {
            lock: SleepLock::new(name),
            data: UnsafeCell::new(data),
        }
    }
    pub fn lock(&self) -> SleepMutexGuard<'_, T> {
        self.lock.acquire();
        SleepMutexGuard { mtx: self }
    }
}
unsafe impl<T: Send> Send for SleepMutex<T> {}
unsafe impl<T: Send> Sync for SleepMutex<T> {}

pub struct SleepMutexGuard<'a, T> {
    mtx: &'a SleepMutex<T>,
}
use core::ops::{Deref, DerefMut};
impl<'a, T> Deref for SleepMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mtx.data.get() }
    }
}
impl<'a, T> DerefMut for SleepMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mtx.data.get() }
    }
}
impl<'a, T> Drop for SleepMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mtx.lock.release();
    }
}
