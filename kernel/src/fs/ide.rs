use super::bcache::BufRef;
use crate::ioapic;
use crate::lock::spin::SpinMutex;
use crate::proc;
use crate::trap;
use alloc::collections::VecDeque;
use lazy_static::lazy_static;
use utils::x86;

lazy_static! {
    static ref IDE_QUEUE: SpinMutex<IdeQueue> = SpinMutex::new("IDE_QUE", IdeQueue::new());
}

struct IdeQueue {
    que: VecDeque<BufRef>,
    running: bool,
}
impl IdeQueue {
    pub fn new() -> Self {
        Self {
            que: VecDeque::new(),
            running: false,
        }
    }
    fn start(&mut self) {
        todo!()
    }
    pub fn append(&mut self, b: BufRef) {
        self.que.push_back(b);
        // Start disk if necessary.
        if self.que.len() == 1 {
            self.start();
        }
    }
    pub fn is_empty(&self) -> bool {
        self.que.is_empty()
    }
}

static mut HAVE_DISK1: bool = false;
fn have_disk1() -> bool {
    unsafe { HAVE_DISK1 }
}

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
    dbg!(have_disk1());

    // Switch back to disk 0
    x86::outb(PORT_BASE + 6, 0xE0 | (0 << 4));
}

pub fn read_from_disk(b: &BufRef) {
    if b.valid() {
        panic!("read_from_disk: nothing to do");
    }
    if b.dev != 0 && !have_disk1() {
        panic!("read_from_disk: ide disk 1 not present");
    }

    IDE_QUEUE.lock().append(b.clone());

    // Wait for read request to finish.
    while !b.valid() {
        todo!(); // sleep
    }
}
pub fn write_to_disk(b: &BufRef) {
    if !b.dirty() {
        panic!("read_from_disk: nothing to do");
    }
    if b.dev != 0 && !have_disk1() {
        panic!("read_from_disk: ide disk 1 not present");
    }

    IDE_QUEUE.lock().append(b.clone());

    // Wait for write request to finish.
    while b.dirty() {
        todo!(); // sleep
    }
}

#[no_mangle]
pub extern "C" fn ide_intr() {
    let mut ide_que = IDE_QUEUE.lock();
    if ide_que.is_empty() {
        return;
    }
    todo!();
}
