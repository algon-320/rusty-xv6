use super::ide;
use super::BLK_SIZE;
use crate::lock::sleep::SleepMutex;
use crate::lock::spin::SpinMutex;
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use core::sync::atomic::{AtomicU8, Ordering};
use lazy_static::lazy_static;

/// buffer has been read from disk
const B_VALID: u8 = 0x2;
/// buffer needs to be written to disk
const B_DIRTY: u8 = 0x4;

pub type BufRef = Arc<Buf>;
pub struct Buf {
    pub dev: u32,
    pub block_no: u32,
    flags: AtomicU8,
    pub data: SleepMutex<[u8; BLK_SIZE]>,
}
impl Buf {
    pub const fn zero() -> Self {
        Self {
            dev: 0,
            block_no: 0,
            flags: AtomicU8::new(0),
            data: SleepMutex::new("buf", [0; BLK_SIZE]),
        }
    }
    pub fn dirty(&self) -> bool {
        (self.flags.load(Ordering::SeqCst) & B_DIRTY) != 0
    }
    pub fn valid(&self) -> bool {
        (self.flags.load(Ordering::SeqCst) & B_VALID) != 0
    }
    pub fn set_flags(&self, flags: u8) {
        self.flags.store(flags, Ordering::SeqCst);
    }
}
impl Drop for Buf {
    fn drop(&mut self) {
        log!("buf drop");
    }
}

struct Bcache {
    cache: BTreeMap<(u32, u32), Weak<Buf>>,
}
impl Bcache {
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
        }
    }
    fn get(&mut self, dev: u32, block_no: u32) -> BufRef {
        let key = (dev, block_no);
        match self.cache.get(&key).and_then(|weak| weak.upgrade()) {
            Some(arc) => arc,
            None => {
                let mut buf = Arc::new(Buf::zero());
                {
                    let buf = Arc::get_mut(&mut buf).unwrap();
                    buf.dev = dev;
                    buf.block_no = block_no;
                    buf.flags = AtomicU8::new(0);
                }
                let weak = Arc::downgrade(&buf);
                self.cache.insert(key, weak);
                buf
            }
        }
    }
}

lazy_static! {
    static ref BCACHE: SpinMutex<Bcache> = SpinMutex::new("bcache", Bcache::new());
}

pub fn read(dev: u32, block_no: u32) -> BufRef {
    let mut bcache = BCACHE.lock();
    let b = bcache.get(dev, block_no);
    if !b.valid() {
        ide::read_from_disk(&b);
    }
    b
}
pub fn write(buf: &BufRef) {
    if buf.dirty() {
        ide::write_to_disk(&buf);
    }
}

pub fn init() {
    lazy_static::initialize(&BCACHE);
}
