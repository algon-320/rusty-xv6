use super::fs;
use super::ioapic;
use super::lock::spin::SpinMutex;
use super::proc;
use super::trap;
use alloc::collections::VecDeque;
use lazy_static::lazy_static;
use utils::x86;

use fs::bcache::BufRef;
lazy_static! {
    static ref IDE_QUEUE: SpinMutex<VecDeque<BufRef>> = SpinMutex::new("IDE_QUE", VecDeque::new());
}

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

pub fn read_from_disk(b: &BufRef) {
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
pub fn write_to_disk(b: &BufRef) {
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
