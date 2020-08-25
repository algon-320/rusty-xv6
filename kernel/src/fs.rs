const BLOCK_SIZE: usize = 512;
pub struct Buf {
    flags: u8,
    dev: u32,
    block_no: u32,
    ref_cnt: u32,
    data: [u8; BLOCK_SIZE],
}
impl Buf {
    pub const fn new() -> Self {
        Self {
            flags: 0,
            dev: 0,
            block_no: 0,
            ref_cnt: 0,
            data: [0; BLOCK_SIZE],
        }
    }
}

pub mod bcache {
    use super::Buf;
    const NBUF: usize = 30;

    #[derive(Debug)]
    struct BufLink {
        next: usize,
        prev: usize,
    }

    use crate::lock::sleep::SleepMutex;
    use crate::lock::spin::SpinMutex;
    struct Bcache {
        // last one is used for the head
        link: SpinMutex<[BufLink; NBUF + 1]>,
        buff: [SleepMutex<Buf>; NBUF + 1],
    }
    impl Bcache {
        pub const fn new() -> Self {
            Self {
                link: SpinMutex::new("bcache", [BufLink { next: 0, prev: 0 }; NBUF + 1]),
                buff: [SleepMutex::new("buffer", Buf::new()); NBUF + 1],
            }
        }
        pub fn init(&self) {
            // initialize self.link
            let mut guard = self.link.lock();
            let link = guard.as_mut();
            let head_idx = NBUF;
            let mut head = BufLink {
                next: head_idx,
                prev: head_idx,
            };
            for i in 0..NBUF {
                link[i].next = head.next;
                link[i].prev = head_idx;
                link[head.next].prev = i;
                head.next = i;
            }
            link[NBUF] = head;
        }
    }

    static BCACHE: Bcache = Bcache::new();

    pub fn init() {
        BCACHE.init();
    }
}

pub mod inode {
    /// in-memory copy of an inode
    pub struct Inode {}
}
