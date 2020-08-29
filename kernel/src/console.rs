use super::fs::file::{init_dev, Dev, CONSOLE};
use super::fs::Result;
use super::lock::spin::SpinLock;

static CONSOLE_LOCK: SpinLock = SpinLock::new("console");

pub fn console_write(buf: &[u8]) -> Result<usize> {
    todo!()
}
pub fn console_read(buf: &mut [u8]) -> Result<usize> {
    todo!()
}

pub fn init() {
    let cons = Dev {
        read: Some(console_read),
        write: Some(console_write),
    };

    unsafe { init_dev(CONSOLE, cons) };
    super::ioapic::enable(super::trap::IRQ_KBD, 0);
}
