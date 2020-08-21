use super::proc::{my_cpu, Cpu};
use core::sync::atomic::{fence, spin_loop_hint, AtomicBool, AtomicPtr, Ordering};
use utils::prelude::*;
use utils::x86;

pub struct SpinLock {
    locked: AtomicBool,

    // for debugging
    name: &'static str,
    cpu: AtomicPtr<Cpu>,
}

impl SpinLock {
    pub const fn new(name: &'static str) -> Self {
        Self {
            locked: AtomicBool::new(false),
            name,
            cpu: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    /// Acquire the lock.
    /// Loops (spins) until the lock is acquired.
    /// Holding a lock for a long time may cause
    /// other CPUs to waste time spinning to acquire it.
    pub fn acquire(&self) {
        push_cli();
        assert!(!self.holding(), "acquire: {}", self.name);
        #[cfg(debug_assertions)]
        log!("lock ({}) acquired", self.name);

        while self.locked.compare_and_swap(false, true, Ordering::Relaxed) {
            spin_loop_hint();
        }

        // Tell the compiler and the processor to not move loads or stores
        // past this point, to ensure that the critical section's memory
        // references happen after the lock is acquired.
        fence(Ordering::Acquire);

        self.cpu.store(my_cpu(), Ordering::Relaxed);
        // TODO: get_caller_pcs

        #[cfg(debug_assertions)]
        log!("lock({}) is now taken by {:p}", self.name, my_cpu());
    }

    // Release the lock.
    pub fn release(&self) {
        assert!(self.holding(), "release: {}", self.name);
        self.cpu.store(core::ptr::null_mut(), Ordering::Relaxed);

        // Tell the compiler and the processor to not move loads or stores
        // past this point, to ensure that all the stores in the critical
        // section are visible to other cores before the lock is released.
        fence(Ordering::Release);

        // Release the lock
        self.locked.store(false, Ordering::Relaxed);

        #[cfg(debug_assertions)]
        log!("lock({}) was released", self.name);
        pop_cli();
    }

    /// Check whether this cpu is holding the lock.
    fn holding(&self) -> bool {
        push_cli();
        let r = self.locked.load(Ordering::Relaxed) && self.cpu.load(Ordering::Relaxed) == my_cpu();
        pop_cli();
        r
    }
}

/// push_cli/pop_cli are like cli/sti except that they are matched:
/// it takes two pop_cli to undo two push_cli.
/// Also, if interrupts are off, then push_cli, pop_cli leaves them off.
fn push_cli() {
    let eflags = x86::read_eflags();
    x86::cli();
    if my_cpu().num_cli == 0 {
        my_cpu().int_enabled = (eflags & x86::FL_IF) != 0;
    }
    my_cpu().num_cli += 1;
}
fn pop_cli() {
    if (x86::read_eflags() & x86::FL_IF) != 0 {
        panic!("pop_cli - interruptible");
    }
    if my_cpu().num_cli == 0 {
        panic!("pop_cli: num_cli zero");
    }
    my_cpu().num_cli -= 1;
    if my_cpu().num_cli == 0 && my_cpu().int_enabled {
        x86::sti();
    }
}

use core::cell::UnsafeCell;
pub struct SpinMutex<T> {
    lock: SpinLock,
    data: UnsafeCell<T>,
}
impl<T> SpinMutex<T> {
    pub const fn new(name: &'static str, data: T) -> Self {
        Self {
            lock: SpinLock::new(name),
            data: UnsafeCell::new(data),
        }
    }
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        self.lock.acquire();
        SpinMutexGuard { mtx: self }
    }
}
unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

pub struct SpinMutexGuard<'a, T> {
    mtx: &'a SpinMutex<T>,
}
use core::ops::{Deref, DerefMut};
impl<'a, T> Deref for SpinMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mtx.data.get() }
    }
}
impl<'a, T> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mtx.data.get() }
    }
}
impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mtx.lock.release();
    }
}
