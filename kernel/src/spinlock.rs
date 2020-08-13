use super::proc;
use core::sync::atomic::{fence, spin_loop_hint, Ordering};
use utils::prelude::*;
use utils::x86;

pub struct SpinLock {
    locked: u32,

    // for debugging
    name: &'static str,
    cpu: Option<&'static proc::Cpu>,
}

impl SpinLock {
    pub fn new(name: &'static str) -> Self {
        Self {
            locked: 0,
            name,
            cpu: None,
        }
    }

    /// Acquire the lock.
    /// Loops (spins) until the lock is acquired.
    /// Holding a lock for a long time may cause
    /// other CPUs to waste time spinning to acquire it.
    pub fn acquire(&mut self) {
        push_cli();
        assert!(!self.holding(), "acquire: {}", self.name);
        log!("lock ({}) acquired", self.name);

        // The xchg is atomic
        while x86::xchgl(&mut self.locked, 1) != 0 {
            spin_loop_hint();
        }

        // Tell the compiler and the processor to not move loads or stores
        // past this point, to ensure that the critical section's memory
        // references happen after the lock is acquired.
        fence(Ordering::Acquire);

        self.cpu = Some(proc::my_cpu());
        // TODO: get_caller_pcs
    }

    // Release the lock.
    pub fn release(&mut self) {
        assert!(self.holding(), "release: {}", self.name);
        self.cpu = None;

        // Tell the compiler and the processor to not move loads or stores
        // past this point, to ensure that all the stores in the critical
        // section are visible to other cores before the lock is released.
        fence(Ordering::Release);

        // Release the lock
        x86::movl0(&mut self.locked);

        log!("lock ({}) released", self.name);
        pop_cli();
    }

    /// Check whether this cpu is holding the lock.
    fn holding(&self) -> bool {
        push_cli();
        let r = self.locked == 1 && self.cpu == Some(proc::my_cpu());
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
    if proc::my_cpu().num_cli == 0 {
        proc::my_cpu().int_enabled = (eflags & x86::FL_IF) != 0;
    }
    proc::my_cpu().num_cli += 1;
}
fn pop_cli() {
    if (x86::read_eflags() & x86::FL_IF) != 0 {
        panic!("pop_cli - interruptible");
    }
    if proc::my_cpu().num_cli == 0 {
        panic!("pop_cli: num_cli zero");
    }
    proc::my_cpu().num_cli -= 1;
    if proc::my_cpu().num_cli == 0 && proc::my_cpu().int_enabled {
        x86::sti();
    }
}
