use super::fs;
use super::ioapic;
use super::lock::spin::SpinMutex;
use super::proc;
use super::trap;
use utils::x86;

use fs::bcache::Buf;
struct IdeQueue {
    next: *const Buf,
}
unsafe impl Send for IdeQueue {}

impl IdeQueue {
    pub const fn new() -> Self {
        Self {
            next: core::ptr::null(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.next.is_null()
    }
    pub fn start(&mut self) {
        todo!()
    }
    pub fn append(&mut self, buf: *const Buf) {
        todo!()
        // unsafe {
        //     // Find last position and put the buf on it
        //     let mut pp: *mut *const Buf = &mut self.next as *mut _;
        //     while !(*pp).is_null() {
        //         pp = (**pp).ide_que_next.get();
        //     }
        //     *pp = buf;
        // }
    }
    pub fn pop(&mut self) -> *const Buf {
        todo!()
        // let p = self.next;
        // if !p.is_null() {
        //     unsafe {
        //         self.next = *(*p).ide_que_next.get();
        //         *(*p).ide_que_next.get() = core::ptr::null();
        //     }
        // }
        // p
    }
}

static IDE_QUEUE: SpinMutex<IdeQueue> = SpinMutex::new("ide", IdeQueue::new());
static mut HAVE_DISK1: bool = false;

const IDE_BSY: u8 = 0x80;
const IDE_DRDY: u8 = 0x40;
const IDE_DF: u8 = 0x20;
const IDE_ERR: u8 = 0x01;

const PORT_BASE: u16 = 0x1F0;

/// Wait for IDE disk to become ready.
fn wait(check_err: bool) -> Option<()> {
    let r = loop {
        let r = x86::inb(PORT_BASE + 7);
        if r & (IDE_BSY | IDE_DRDY) == IDE_DRDY {
            break r;
        }
    };
    if check_err && (r & (IDE_DF | IDE_ERR)) > 0 {
        None
    } else {
        Some(())
    }
}

pub fn init() {
    let last_cpu = proc::cpus().len() - 1;
    ioapic::enable(trap::IRQ_IDE, last_cpu);
    wait(false);

    // Check if disk 1 is present
    x86::outb(PORT_BASE + 6, 0xE0 | (1 << 4));
    for _ in 0..1000 {
        if x86::inb(PORT_BASE + 7) != 0 {
            unsafe { HAVE_DISK1 = true };
            break;
        }
    }
    unsafe { dbg!(HAVE_DISK1) };

    // Switch back to disk 0
    x86::outb(PORT_BASE + 6, 0xE0 | (0 << 4));
}

pub fn read_from_disk(b: &Buf) {
    todo!()
    // if b.valid() {
    //     panic!("read_from_disk: nothing to do");
    // }
    // if b.dev != 0 && unsafe { !HAVE_DISK1 } {
    //     panic!("read_from_disk: ide disk 1 not present");
    // }

    // let mut ide_que = IDE_QUEUE.lock();
    // ide_que.append(b);

    // // Start disk if necessary.
    // if ide_que.next == b as *const _ {
    //     ide_que.start();
    // }
    // // Wait for read request to finish.
    // while !b.valid() {
    //     todo!(); // sleep
    // }
}
pub fn write_to_disk(b: &Buf) {
    todo!()
    // if !b.dirty() {
    //     panic!("read_from_disk: nothing to do");
    // }
    // if b.dev != 0 && unsafe { !HAVE_DISK1 } {
    //     panic!("read_from_disk: ide disk 1 not present");
    // }

    // let mut ide_que = IDE_QUEUE.lock();
    // ide_que.append(b);

    // // Start disk if necessary.
    // if ide_que.next == b as *const _ {
    //     ide_que.start();
    // }
    // // Wait for write request to finish.
    // while b.dirty() {
    //     todo!(); // sleep
    // }
    // todo!()
}

#[no_mangle]
pub extern "C" fn ide_intr() {
    let mut ide_que = IDE_QUEUE.lock();
    if ide_que.is_empty() {
        return;
    }
    todo!();
}
