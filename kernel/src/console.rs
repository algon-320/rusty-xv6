use super::spinlock::SpinLock;

static CONSOLE_LOCK: SpinLock = SpinLock::new("console");

/// in-memory copy of an inode
pub struct Inode {}

/// table mapping major device number to device functions
struct Dev {
    pub read: fn(&'static Inode, &mut [u8]) -> i32,
    pub write: fn(&'static Inode, &[u8]) -> i32,
}

const NDEV: usize = 10;
const CONSOLE: usize = 1;
static mut DEV: [Option<Dev>; NDEV] = [None, None, None, None, None, None, None, None, None, None];

pub fn console_write(inode: &'static Inode, buf: &[u8]) -> i32 {
    todo!()
}
pub fn console_read(inode: &'static Inode, buf: &mut [u8]) -> i32 {
    todo!()
}

pub fn init() {
    let cons = Dev {
        read: console_read,
        write: console_write,
    };

    unsafe { DEV[CONSOLE] = Some(cons) };
    super::ioapic::enable(super::trap::IRQ_KBD, 0);
}
