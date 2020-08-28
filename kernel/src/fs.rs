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

const N_DIRECT: usize = 12;
const N_INDIRECT: usize = BLOCK_SIZE / core::mem::size_of::<u32>();

// Directory is a file containing a sequence of dirent structures.
const DIR_SIZE: usize = 14;

pub mod inode {
    use crate::lock::sleep::SleepMutex;
    use crate::lock::spin::SpinMutex;
    use crate::proc::my_proc;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use utils::prelude::*;

    const ROOT_DEV: usize = 1;
    const ROOT_INO: usize = 1;

    /// maximum number of active i-nodes
    const N_INODE: usize = 50;
    static ICACHE: SpinMutex<[Inode; N_INODE]> = SpinMutex::new("icache", [Inode::zero(); N_INODE]);

    /// in-memory copy of an inode
    pub struct Inode {
        dev: usize,           // Device number
        inum: usize,          // Inode number
        ref_cnt: AtomicUsize, // Reference count
        body: SleepMutex<InodeBody>,
    }
    impl Inode {
        pub const fn zero() -> Self {
            Self {
                dev: 0,
                inum: 0,
                ref_cnt: AtomicUsize::new(0),
                body: SleepMutex::new("inode", InodeBody::zero()),
            }
        }
        pub fn get_ref(&self) -> InodeRef {
            self.ref_cnt.fetch_add(1, Ordering::SeqCst);
            InodeRef { ptr: self }
        }
    }
    pub struct InodeBody {
        valid: bool,
        type_: u16,
        major: u16,
        minor: u16,
        nlink: u16,
        size: usize,
        addrs: [u32; super::N_DIRECT + 1],
    }
    impl InodeBody {
        pub const fn zero() -> Self {
            Self {
                valid: false,
                type_: 0,
                major: 0,
                minor: 0,
                nlink: 0,
                size: 0,
                addrs: [0; super::N_DIRECT + 1],
            }
        }
    }

    pub struct InodeRef {
        ptr: *const Inode,
    }
    impl InodeRef {
        pub const fn dangling() -> Self {
            Self {
                ptr: core::ptr::null(),
            }
        }
        pub fn dup(&self) -> Self {
            let _guard = ICACHE.lock();
            assert!(!self.ptr.is_null(), "dup on dangling InodeRef");
            unsafe { (*self.ptr).ref_cnt.fetch_add(1, Ordering::SeqCst) };
            Self { ptr: self.ptr }
        }
    }
    impl Drop for InodeRef {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                let _guard = ICACHE.lock();
                log!("InodeRef drop");
                unsafe { (*self.ptr).ref_cnt.fetch_sub(1, Ordering::SeqCst) };
            }
        }
    }

    fn init() {
        todo!()
    }

    fn get(dev: usize, inum: usize) -> InodeRef {
        let mut icache = ICACHE.lock();

        let mut empty: *mut Inode = core::ptr::null_mut();
        for ip in icache.iter_mut() {
            if ip.ref_cnt.load(Ordering::SeqCst) > 0 && ip.dev == dev && ip.inum == inum {
                return ip.get_ref(); // ref_cnt += 1
            }
            if empty.is_null() && ip.ref_cnt.load(Ordering::SeqCst) == 0 {
                empty = ip;
            }
        }
        assert!(!empty.is_null(), "inode::get: no inodes");

        let ip = unsafe { &mut *empty };
        ip.dev = dev;
        ip.inum = inum;
        ip.ref_cnt = AtomicUsize::new(0);
        ip.body.lock().valid = false;
        ip.get_ref() // ref_cnt -> 1
    }

    fn name_x(path: &str, name_iparent: bool, name: &mut [u8]) -> InodeRef {
        let ip = match path {
            "/" => get(ROOT_DEV, ROOT_INO),
            _ => unsafe { (*my_proc()).cwd.dup() },
        };
        todo!()
    }
    pub fn from_name(path: &str) -> InodeRef {
        let mut name = [0; super::DIR_SIZE];
        name_x(path, true, &mut name)
    }
}
