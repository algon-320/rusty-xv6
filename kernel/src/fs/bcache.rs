use super::ide;
use super::BLK_SIZE;
use crate::lock::sleep::{SleepMutex, SleepMutexGuard};
use crate::lock::spin::SpinMutex;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU8, Ordering};
use lazy_static::lazy_static;

/// buffer has been read from disk
const B_VALID: u8 = 0x2;
/// buffer needs to be written to disk
const B_DIRTY: u8 = 0x4;

pub struct Flags(AtomicU8);
impl Flags {
    pub fn empty() -> Self {
        Self(AtomicU8::new(0))
    }

    pub fn set_dirty(&self, dirty: bool) {
        if dirty {
            self.0.fetch_or(B_DIRTY, Ordering::SeqCst);
        } else {
            self.0.fetch_and(!B_DIRTY, Ordering::SeqCst);
        }
    }
    pub fn dirty(&self) -> bool {
        self.0.load(Ordering::SeqCst) & B_DIRTY != 0
    }

    pub fn set_valid(&self, valid: bool) {
        if valid {
            self.0.fetch_or(B_VALID, Ordering::SeqCst);
        } else {
            self.0.fetch_and(!B_VALID, Ordering::SeqCst);
        }
    }
    pub fn valid(&self) -> bool {
        self.0.load(Ordering::SeqCst) & B_VALID != 0
    }
}

pub struct BufLocked {
    guard: SleepMutexGuard<'static, Buf>,
}
impl core::ops::Deref for BufLocked {
    type Target = Buf;
    fn deref(&self) -> &Self::Target {
        &*self.guard
    }
}
impl core::ops::DerefMut for BufLocked {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.guard
    }
}
impl Drop for BufLocked {
    fn drop(&mut self) {
        unsafe { BCACHE.lock().put(self.dev, self.block_no) };
    }
}

pub struct Buf {
    pub dev: u32,
    pub block_no: u32,
    pub flags: Flags,
    pub data: [u8; BLK_SIZE],
}
impl Buf {
    pub fn zero() -> Self {
        Self {
            dev: 0,
            block_no: 0,
            flags: Flags::empty(),
            data: [0; BLK_SIZE],
        }
    }
    pub fn id(&self) -> usize {
        self as *const _ as usize
    }
    pub fn write(&self) {
        if self.flags.dirty() {
            ide::write_to_disk(self);
        }
        debug_assert!(!self.flags.dirty());
    }
}
impl Drop for Buf {
    fn drop(&mut self) {
        log!("buf drop");
    }
}

struct Bcache {
    cache: BTreeMap<(u32, u32), (usize, &'static SleepMutex<Buf>)>,
}
impl Bcache {
    pub fn new() -> Self {
        Self {
            cache: BTreeMap::new(),
        }
    }
    fn get(&mut self, dev: u32, block_no: u32) -> &'static SleepMutex<Buf> {
        let key = (dev, block_no);
        match self.cache.get_mut(&key) {
            Some((ref_cnt, r)) => {
                *ref_cnt += 1;
                r
            }
            None => {
                let buf = {
                    let mut buf = Buf::zero();
                    buf.dev = dev;
                    buf.block_no = block_no;
                    buf
                };
                let buf = Box::leak(Box::new(SleepMutex::new("buf", buf)));
                self.cache.insert(key, (1, buf));
                buf
            }
        }
    }
    unsafe fn put(&mut self, dev: u32, block_no: u32) {
        let key = (dev, block_no);
        match self.cache.get_mut(&key) {
            Some((ref_cnt, mtx)) => {
                *ref_cnt -= 1;
                if *ref_cnt == 0 {
                    // Retrieve the box and drop it.
                    drop(Box::from_raw(mtx));
                    self.cache.remove(&key);
                }
            }
            None => panic!("Bcache::put: no entry"),
        }
    }
}

lazy_static! {
    static ref BCACHE: SpinMutex<Bcache> = SpinMutex::new("bcache", Bcache::new());
}

pub fn read(dev: u32, block_no: u32) -> BufLocked {
    let b = BCACHE.lock().get(dev, block_no);
    let mut b = BufLocked { guard: b.lock() };
    if !b.flags.valid() {
        ide::read_from_disk(&mut b);
    }
    debug_assert!(b.flags.valid());
    b
}

pub fn init() {
    lazy_static::initialize(&BCACHE);
}
