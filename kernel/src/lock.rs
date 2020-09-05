pub mod spin {
    use crate::lapic::lapic_id;
    use crate::proc::my_cpu_id;
    use core::sync::atomic::{fence, spin_loop_hint, AtomicBool, AtomicI8, Ordering};

    pub struct SpinLock {
        locked: AtomicBool,

        // for debugging
        name: &'static str,
        cpu: AtomicI8,
    }

    impl SpinLock {
        pub const fn new(name: &'static str) -> Self {
            Self {
                locked: AtomicBool::new(false),
                name,
                cpu: AtomicI8::new(-1),
            }
        }

        /// Acquire the lock.
        /// Loops (spins) until the lock is acquired.
        /// Holding a lock for a long time may cause
        /// other CPUs to waste time spinning to acquire it.
        pub fn acquire(&self) {
            if lapic_id().is_none() {
                return;
            }
            super::push_cli();
            assert!(!self.holding(), "acquire: {}", self.name);

            while self.locked.compare_and_swap(false, true, Ordering::Relaxed) {
                spin_loop_hint();
            }

            // Tell the compiler and the processor to not move loads or stores
            // past this point, to ensure that the critical section's memory
            // references happen after the lock is acquired.
            fence(Ordering::Acquire);

            self.cpu.store(my_cpu_id() as i8, Ordering::Relaxed);
            // TODO: get_caller_pcs
        }

        // Release the lock.
        pub fn release(&self) {
            if lapic_id().is_none() {
                return;
            }
            assert!(self.holding(), "release: {}", self.name);
            self.cpu.store(-1, Ordering::Relaxed);

            // Tell the compiler and the processor to not move loads or stores
            // past this point, to ensure that all the stores in the critical
            // section are visible to other cores before the lock is released.
            fence(Ordering::Release);

            // Release the lock
            self.locked.store(false, Ordering::Relaxed);
            super::pop_cli();
        }

        /// Check whether this cpu is holding the lock.
        fn holding(&self) -> bool {
            super::cli(|| {
                self.locked.load(Ordering::Relaxed)
                    && self.cpu.load(Ordering::Relaxed) == my_cpu_id() as i8
            })
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
}

pub mod sleep {
    use super::spin::SpinMutex;

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
}

use super::proc::my_cpu;
use utils::x86;

/// push_cli/pop_cli are like cli/sti except that they are matched:
/// it takes two pop_cli to undo two push_cli.
/// Also, if interrupts are off, then push_cli, pop_cli leaves them off.
pub fn push_cli() {
    let eflags = x86::read_eflags();
    x86::cli();
    let mut cpu = my_cpu();
    if cpu.num_cli == 0 {
        cpu.int_enabled = (eflags & x86::FL_IF) != 0;
    }
    cpu.num_cli += 1;
}
pub fn pop_cli() {
    if (x86::read_eflags() & x86::FL_IF) != 0 {
        panic!("pop_cli - interruptible");
    }
    let mut cpu = my_cpu();
    if cpu.num_cli == 0 {
        panic!("pop_cli: num_cli zero");
    }
    cpu.num_cli -= 1;
    if cpu.num_cli == 0 && cpu.int_enabled {
        x86::sti();
    }
}
pub fn cli<F, R>(f: F) -> R
where
    F: Fn() -> R,
{
    push_cli();
    let r = f();
    pop_cli();
    r
}
