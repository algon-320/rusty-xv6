use super::bcache::{Buf, Flags};
use super::BLK_SIZE;
use crate::ioapic;
use crate::lock::spin::SpinMutex;
use crate::proc;
use crate::trap;
use alloc::collections::VecDeque;
use lazy_static::lazy_static;
use utils::x86;

const SECTOR_SIZE: usize = 512;

const IDE_BSY: u8 = 0x80;
const IDE_DRDY: u8 = 0x40;
const IDE_DF: u8 = 0x20;
const IDE_ERR: u8 = 0x01;

const IDE_CMD_READ: u8 = 0x20;
const IDE_CMD_WRITE: u8 = 0x30;
const IDE_CMD_RDMUL: u8 = 0xC4;
const IDE_CMD_WRMUL: u8 = 0xC5;

const PORT_BASE: u16 = 0x1F0;

/// buffer has been read from disk
const B_VALID: u8 = 0x2;
/// buffer needs to be written to disk
const B_DIRTY: u8 = 0x4;

static mut HAVE_DISK1: bool = false;
fn have_disk1() -> bool {
    unsafe { HAVE_DISK1 }
}

lazy_static! {
    static ref IDE_QUEUE: SpinMutex<IdeQueue> = SpinMutex::new("IDE_QUE", IdeQueue::new());
}

#[derive(PartialEq, Eq)]
enum Command {
    Read,
    Write,
}
struct Request {
    cmd: Command,
    dev: u32,
    block_no: u32,
    data: *mut u8,
    flags: *const Flags,
    wait_chan: usize,
}
unsafe impl Send for Request {}

struct IdeQueue {
    que: VecDeque<Request>,
    running: bool,
}
impl IdeQueue {
    pub fn new() -> Self {
        Self {
            que: VecDeque::new(),
            running: false,
        }
    }

    pub fn append(&mut self, req: Request) {
        self.que.push_back(req);
        // Start disk if necessary.
        if self.que.len() == 1 {
            self.start();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.que.is_empty()
    }

    fn start(&mut self) {
        let req = self.que.front().unwrap();

        let sector_per_block = BLK_SIZE / SECTOR_SIZE;
        let sector = req.block_no as usize * sector_per_block;
        let read_cmd = if sector_per_block == 1 {
            IDE_CMD_READ
        } else {
            IDE_CMD_RDMUL
        };
        let write_cmd = if sector_per_block == 1 {
            IDE_CMD_WRITE
        } else {
            IDE_CMD_WRMUL
        };
        assert!(sector_per_block <= 7, "IdeQueue::start");

        wait(false);
        x86::outb(0x3F6, 0); // generate interrupt
        x86::outb(0x1F2, sector_per_block as u8); // number of sectors
        x86::outb(0x1F3, (sector & 0xFF) as u8);
        x86::outb(0x1F4, ((sector >> 8) & 0xFF) as u8);
        x86::outb(0x1F5, ((sector >> 16) & 0xFF) as u8);
        x86::outb(
            0x1F6,
            0xE0 | (((req.dev & 1) << 4) as u8) | ((sector >> 24) & 0x0F) as u8,
        );
        match req.cmd {
            Command::Read => {
                x86::outb(0x1F0, read_cmd);
            }
            Command::Write => {
                x86::outb(0x1F7, write_cmd);
                x86::outsl(0x1F0, req.data as *const u32, BLK_SIZE / 4);
            }
        }

        self.running = true;
    }

    fn notify_ready(&mut self) {
        self.running = false;
        let req = self.que.pop_front().unwrap();

        // Read data if needed.
        if req.cmd == Command::Read && wait(true).is_some() {
            x86::insl(0x1F0, req.data as *mut u32, BLK_SIZE / 4);
        }

        // Wake process waiting for this buf.
        // clear B_DIRTY and set B_VALID
        unsafe { (*req.flags).set_dirty(false) };
        unsafe { (*req.flags).set_valid(true) };
        proc::wakeup(req.wait_chan);

        // Start disk on next buf in queue.
        if !self.is_empty() {
            self.start();
        }
    }
}

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

pub fn read_from_disk(b: &mut Buf) {
    assert!(!b.flags.valid(), "read_from_disk: nothing to do");
    if b.dev != 0 {
        assert!(have_disk1(), "read_from_disk: ide disk 1 not present");
    }

    let mut ide_que = IDE_QUEUE.lock();
    let req = Request {
        cmd: Command::Read,
        dev: b.dev,
        block_no: b.block_no,
        data: b.data.as_mut_ptr(),
        flags: &b.flags,
        wait_chan: b.id(),
    };
    ide_que.append(req);

    // Wait for read request to finish.
    while !b.flags.valid() {
        proc::sleep(b.id(), &ide_que);
    }
}
pub fn write_to_disk(b: &Buf) {
    assert!(b.flags.dirty(), "read_from_disk: nothing to do");
    if b.dev != 0 {
        assert!(have_disk1(), "read_from_disk: ide disk 1 not present");
    }

    let mut ide_que = IDE_QUEUE.lock();
    let req = Request {
        cmd: Command::Read,
        dev: b.dev,
        block_no: b.block_no,
        data: b.data.as_ptr() as *const _ as *mut _,
        flags: &b.flags,
        wait_chan: b.id(),
    };
    ide_que.append(req);

    // Wait for write request to finish.
    while b.flags.dirty() {
        proc::sleep(b.id(), &ide_que);
    }
}

/// Interrupt handler.
#[no_mangle]
pub extern "C" fn ide_intr() {
    let mut ide_que = IDE_QUEUE.lock();
    if !ide_que.is_empty() {
        ide_que.notify_ready();
    }
}
