const BLK_SIZE: usize = 512;

pub trait DropRef {
    fn drop_ref(&'static self);
}

#[repr(transparent)]
pub struct StaticRef<T: 'static + DropRef> {
    ptr: &'static T,
}
impl<T: 'static + DropRef> StaticRef<T> {
    pub fn new(ptr: &'static T) -> Self {
        Self { ptr }
    }
}
impl<T: 'static + DropRef> core::ops::Deref for StaticRef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.ptr
    }
}
impl<T: 'static + DropRef> Drop for StaticRef<T> {
    fn drop(&mut self) {
        self.ptr.drop_ref();
    }
}

pub mod bcache {
    use super::BLK_SIZE;
    use super::{DropRef, StaticRef};
    use crate::ide;
    use crate::lock::sleep::SleepMutex;
    use crate::lock::spin::SpinMutex;
    use core::ptr::null_mut;
    use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

    /// buffer has been read from disk
    const B_VALID: u8 = 0x2;
    /// buffer needs to be written to disk
    const B_DIRTY: u8 = 0x4;

    pub struct Buf {
        idx: usize,
        pub dev: u32,
        pub block_no: u32,
        ref_cnt: AtomicUsize,
        flags: AtomicU8,
        pub body: SleepMutex<BufBody>,
    }
    pub struct BufBody {
        pub data: [u8; BLK_SIZE],
    }
    impl Buf {
        pub const fn zero() -> Self {
            Self {
                idx: usize::MAX,
                dev: 0,
                block_no: 0,
                ref_cnt: AtomicUsize::new(0),
                flags: AtomicU8::new(0),
                body: SleepMutex::new(
                    "buf",
                    BufBody {
                        data: [0; BLK_SIZE],
                    },
                ),
            }
        }

        fn increment_ref_cnt(&self) {
            self.ref_cnt.fetch_add(1, Ordering::SeqCst);
        }
        fn get_ref_cnt(&self) -> usize {
            self.ref_cnt.load(Ordering::SeqCst)
        }
        fn decrement_ref_cnt(&self) {
            self.ref_cnt.fetch_sub(1, Ordering::SeqCst);
        }

        pub fn unused(&self) -> bool {
            // Enven if ref_cnt == 0, B_DIRTY indicates a buffer is in use
            // because the log module has modified it but not yet committed it.
            self.get_ref_cnt() == 0 && !self.dirty()
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

        pub fn dup(&'static self) -> BufRef {
            self.increment_ref_cnt();
            BufRef::new(self)
        }
    }
    unsafe impl Send for Buf {}

    impl DropRef for Buf {
        fn drop_ref(&'static self) {
            self.decrement_ref_cnt();
            if self.get_ref_cnt() == 0 {
                release_buf(self);
            }
        }
    }
    pub type BufRef = StaticRef<Buf>;

    type BufNode = (BufLink, Buf);
    struct BufLink {
        prev: *mut BufNode,
        next: *mut BufNode,
    }
    impl BufLink {
        const fn zero() -> Self {
            Self {
                prev: null_mut(),
                next: null_mut(),
            }
        }
    }
    struct Bcache {
        arena: *mut [BufNode; N_BUF],
        unused: *mut BufNode,
        used: *mut BufNode,
    }
    impl Bcache {
        pub const fn zero() -> Self {
            Self {
                arena: null_mut(),
                unused: null_mut(),
                used: null_mut(),
            }
        }
        fn arena(&self, idx: usize) -> &'static mut BufNode {
            unsafe { &mut (*self.arena)[idx] }
        }
        pub fn init(&mut self) {
            static mut BCACHE_ARENA: [BufNode; N_BUF] = [(BufLink::zero(), Buf::zero()); N_BUF];
            self.arena = unsafe { &mut BCACHE_ARENA };

            for i in 0..N_BUF {
                self.arena(i).1.idx = i;
                self.arena(i).0.prev = if i > 0 {
                    self.arena(i - 1) as *mut BufNode
                } else {
                    null_mut()
                };
                self.arena(i).0.next = if i < N_BUF - 1 {
                    self.arena(i + 1) as *mut BufNode
                } else {
                    null_mut()
                };
            }
            self.unused = self.arena(0) as *mut BufNode;
        }
        fn search_cached(&self, dev: u32, block_no: u32) -> Option<BufRef> {
            // Is the block already cached?
            let mut p: *const BufNode = self.used;
            while !p.is_null() {
                let (l, b) = unsafe { &*p };
                if b.dev == dev && b.block_no == block_no {
                    return Some(b.dup());
                }
                p = l.next as *const _;
            }
            None
        }
        fn take_unused(&mut self) -> Option<&'static mut Buf> {
            if self.unused.is_null() {
                None
            } else {
                unsafe {
                    let node = &mut *self.unused;
                    assert_eq!(node.0.prev, null_mut());

                    let next = node.0.next;
                    self.unused = next;
                    if !next.is_null() {
                        (*next).0.prev = null_mut();
                    }

                    node.0.next = self.used;
                    if !self.used.is_null() {
                        (*self.used).0.prev = node as *mut BufNode;
                    }
                    self.used = node as *mut BufNode;

                    Some(&mut node.1)
                }
            }
        }
        fn get(&mut self, dev: u32, block_no: u32) -> BufRef {
            if let Some(buf) = self.search_cached(dev, block_no) {
                return buf;
            }
            if let Some(buf) = self.take_unused() {
                buf.dev = dev;
                buf.block_no = block_no;
                buf.ref_cnt = AtomicUsize::new(0);
                buf.flags = AtomicU8::new(0);
                return buf.dup();
            }
            panic!("Bcache::get: no buffers")
        }
        fn release(&mut self, buf: &'static Buf) {
            assert_eq!(buf.get_ref_cnt(), 0);
            let node = self.arena(buf.idx);
            unsafe {
                let p = node.0.prev;
                let n = node.0.next;
                if !p.is_null() {
                    (*p).0.next = n;
                } else {
                    self.used = n;
                }
                if !n.is_null() {
                    (*n).0.prev = p;
                }

                if !self.unused.is_null() {
                    (*self.unused).0.prev = node as *mut BufNode;
                }
                node.0.prev = null_mut();
                node.0.next = self.unused;
                self.unused = node as *mut BufNode;
            }
        }
    }
    unsafe impl Send for Bcache {}

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

    fn release_buf(buf: &'static Buf) {
        BCACHE.lock().release(buf);
    }

    const N_BUF: usize = 30;
    static BCACHE: SpinMutex<Bcache> = SpinMutex::new("bcache", Bcache::zero());

    pub fn init() {
        BCACHE.lock().init();
    }

    #[test_case]
    fn test_bcache() {
        init();
        assert!(BCACHE.lock().used.is_null());

        let b1 = BCACHE.lock().get(0, 1);
        assert_eq!(b1.get_ref_cnt(), 1);
        let (_, x) = unsafe { &*BCACHE.lock().used };
        assert_eq!(x.block_no, 1);
        let b2 = BCACHE.lock().get(0, 2);
        assert_eq!(b2.get_ref_cnt(), 1);
        let (_, x) = unsafe { &*BCACHE.lock().used };
        assert_eq!(x.block_no, 2);
        let b3 = BCACHE.lock().get(0, 3);
        assert_eq!(b3.get_ref_cnt(), 1);
        let (_, x) = unsafe { &*BCACHE.lock().used };
        assert_eq!(x.block_no, 3);

        // used --> [b3] <--> [b2] <--> [b1]

        drop(b2);

        // used --> [b3] <--> [b1]
        let (_, b) = unsafe { &*BCACHE.lock().unused };
        let (_, x) = unsafe { &*BCACHE.lock().used };
        assert_eq!(b3.ptr as *const _, x as *const _);
        assert_eq!(b.block_no, 2);
        assert_eq!(x.block_no, 3);

        drop(b3);

        // used --> [b1]
        let (_, b) = unsafe { &*BCACHE.lock().unused };
        let (_, x) = unsafe { &*BCACHE.lock().used };
        assert_eq!(b1.ptr as *const _, x as *const _);
        assert_eq!(b.block_no, 3);
        assert_eq!(x.block_no, 1);

        drop(b1);
        let (_, b) = unsafe { &*BCACHE.lock().unused };
        assert_eq!(b.block_no, 1);

        assert!(BCACHE.lock().used.is_null());
    }
}

const N_DIRECT: usize = 12;
const N_INDIRECT: usize = BLK_SIZE / core::mem::size_of::<u32>();

// Directory is a file containing a sequence of dirent structures.
const DIR_SIZE: usize = 14;

type BlockNum = u32;
struct SuperBlock {
    /// Size of file system image (blocks)
    size: usize,
    /// Number of data blocks
    nblocks: usize,
    /// Number of inodes
    ninodes: usize,
    /// Number of log blocks
    nlog: usize,
    /// Block number of first log block
    log_start: BlockNum,
    /// Block number of first inode block
    inode_start: BlockNum,
    /// Block number of first free map block
    bmap_start: BlockNum,
}
impl SuperBlock {
    pub fn read(&mut self, dev: usize) {
        todo!()
    }
}

#[repr(C)]
struct DirEnt {
    inum: u16,
    name: [u8; DIR_SIZE],
}
impl DirEnt {
    pub const fn zero() -> Self {
        Self {
            inum: 0,
            name: [0; DIR_SIZE],
        }
    }
}

pub fn free_disk_block(dev: u32, block_no: u32) {
    todo!()
}

pub mod inode {
    use super::bcache;
    use super::file;
    use super::free_disk_block;
    use super::DirEnt;
    use super::{DropRef, StaticRef};
    use super::{Error, Result};
    use crate::lock::sleep::SleepMutex;
    use crate::lock::spin::SpinMutex;
    use crate::proc::my_proc;
    use core::cell::UnsafeCell;
    use core::ptr::null_mut;
    use core::sync::atomic::{AtomicUsize, Ordering};

    const ROOT_DEV: u32 = 1;
    const ROOT_INO: u32 = 1;

    /// in-memory copy of an inode
    pub struct Inode {
        prev: UnsafeCell<*mut Inode>,
        next: UnsafeCell<*mut Inode>,

        dev: u32,             // Device number
        inum: u32,            // Inode number
        ref_cnt: AtomicUsize, // Reference count
        body: SleepMutex<InodeBody>,
    }
    impl Inode {
        pub const fn zero() -> Self {
            Self {
                prev: UnsafeCell::new(null_mut()),
                next: UnsafeCell::new(null_mut()),

                dev: 0,
                inum: 0,
                ref_cnt: AtomicUsize::new(0),
                body: SleepMutex::new("inode", InodeBody::zero()),
            }
        }
        fn increment_ref_cnt(&self) {
            self.ref_cnt.fetch_add(1, Ordering::SeqCst);
        }
        fn get_ref_cnt(&self) -> usize {
            self.ref_cnt.load(Ordering::SeqCst)
        }
        fn decrement_ref_cnt(&self) {
            self.ref_cnt.fetch_sub(1, Ordering::SeqCst);
        }
        pub fn dup(&'static self) -> InodeRef {
            self.increment_ref_cnt();
            InodeRef::new(self)
        }

        /// Truncate inode (discard contents).
        /// Only called when the inode has no links to it
        /// (no directory entries referring to it)
        /// and has no in-memory reference to it
        /// (is not an open file or current directory).
        fn trunc(&self) {
            use super::{N_DIRECT, N_INDIRECT};

            let mut body = self.body.lock();
            for addr in body.addrs[..N_DIRECT].iter_mut() {
                if *addr != 0 {
                    free_disk_block(self.dev, *addr);
                    *addr = 0;
                }
            }
            let indirect = body.addrs[N_DIRECT];
            if indirect != 0 {
                let b = bcache::read(self.dev, indirect);
                {
                    let body = b.body.lock();
                    let slots =
                        unsafe { *(body.data.as_ptr() as *const _ as *const [u32; N_INDIRECT]) };
                    for addr in slots.iter() {
                        if *addr != 0 {
                            free_disk_block(self.dev, *addr);
                        }
                    }
                    free_disk_block(self.dev, indirect);
                }
                body.addrs[N_DIRECT] = 0;
            }
            body.size = 0;
            todo!(); // iupdate
        }
    }
    unsafe impl Send for Inode {}
    unsafe impl Sync for Inode {}

    pub struct InodeBody {
        valid: bool,
        type_: FileType,
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
                type_: FileType::Invalid,
                major: 0,
                minor: 0,
                nlink: 0,
                size: 0,
                addrs: [0; super::N_DIRECT + 1],
            }
        }

        /// Read data from inode.
        fn read(&self, dst: &mut [u8], off: usize) -> Result<usize> {
            if self.type_ == FileType::Device {
                assert!((self.major as usize) < file::N_DEV);
                let read = file::dev()[self.major as usize]
                    .read
                    .expect("read function undefined");
                read(dst)
            } else {
                let n = dst.len();
                if off > self.size || off.wrapping_add(n) < off {
                    return Err(Error::InvalidArg("offset"));
                }
                todo!()
            }
        }
        fn write(&self, src: &[u8], off: usize) -> Result<usize> {
            todo!()
        }
    }

    impl DropRef for Inode {
        fn drop_ref(&'static self) {
            log!("InodeRef drop");
            let mut body = self.body.lock();

            self.decrement_ref_cnt();
            if body.valid && body.nlink == 0 {
                if self.get_ref_cnt() == 0 {
                    // inode has no links and no other references: truncate and free.
                    // TODO: trunc
                    body.type_ = FileType::Invalid;
                    // TODO: update
                    body.valid = false;
                    todo!();

                    ICACHE.lock().release(self);
                }
            }
        }
    }
    pub type InodeRef = StaticRef<Inode>;

    pub struct Icache {
        unused: *mut Inode,
        used: *mut Inode,
    }
    impl Icache {
        pub const fn zero() -> Self {
            Self {
                unused: null_mut(),
                used: null_mut(),
            }
        }
        pub fn init(&mut self) {
            static mut ICACHE_ARENA: [Inode; N_INODE] = [Inode::zero(); N_INODE];
            for i in 0..N_INODE {
                unsafe {
                    *ICACHE_ARENA[i].prev.get() = if i > 0 {
                        &mut ICACHE_ARENA[i - 1]
                    } else {
                        null_mut()
                    };
                    *ICACHE_ARENA[i].next.get() = if i < N_INODE - 1 {
                        &mut ICACHE_ARENA[i + 1]
                    } else {
                        null_mut()
                    };
                }
            }
            self.unused = unsafe { &mut ICACHE_ARENA[0] };
        }
        fn search_cached(&self, dev: u32, inum: u32) -> Option<InodeRef> {
            let mut p = self.used;
            while !p.is_null() {
                let i = unsafe { &*p };
                if i.dev == dev && i.inum == inum {
                    return Some(i.dup());
                }
                p = unsafe { *i.next.get() };
            }
            None
        }
        fn take_unused(&mut self) -> Option<&'static mut Inode> {
            if self.unused.is_null() {
                None
            } else {
                unsafe {
                    let ip = &mut *self.unused;
                    assert_eq!(*ip.prev.get(), null_mut());

                    let next = *ip.next.get();
                    self.unused = next;
                    if !next.is_null() {
                        *(*next).prev.get() = null_mut();
                    }

                    *ip.next.get() = self.used;
                    if !self.used.is_null() {
                        *(*self.used).prev.get() = ip;
                    }
                    self.used = ip;

                    Some(ip)
                }
            }
        }
        pub fn get(&mut self, dev: u32, inum: u32) -> InodeRef {
            self.search_cached(dev, inum)
                .or_else(|| {
                    let ip = self.take_unused()?;
                    ip.dev = dev;
                    ip.inum = inum;
                    ip.ref_cnt = AtomicUsize::new(0);
                    Some(ip.dup())
                })
                .expect("inode::get: no inodes")
        }
        fn release(&mut self, ip: &'static Inode) {
            assert_eq!(ip.get_ref_cnt(), 0);
            unsafe {
                let p = *ip.prev.get();
                let n = *ip.next.get();
                if !p.is_null() {
                    *(*p).next.get() = n;
                } else {
                    self.used = n;
                }
                if !n.is_null() {
                    *(*n).prev.get() = p;
                }

                if !self.unused.is_null() {
                    *(*self.unused).prev.get() = ip as *const _ as *mut _;
                }
                *ip.prev.get() = null_mut();
                *ip.next.get() = self.unused;
                self.unused = ip as *const _ as *mut _;
            }
        }
    }
    unsafe impl Send for Icache {}

    /// maximum number of active i-nodes
    const N_INODE: usize = 50;
    static ICACHE: SpinMutex<Icache> = SpinMutex::new("icache", Icache::zero());

    #[derive(Debug, Eq, PartialEq)]
    #[repr(u16)]
    enum FileType {
        Invalid = 0,
        Directory = 1,
        File = 2,
        Device = 3,
    }

    struct Stat {
        type_: FileType, // Type of file
        dev: u32,        // File system's disk device
        ino: usize,      // Inode number
        nlink: u16,      // Number of links to file
        size: usize,     // Size of file in bytes
    }

    pub fn init() {
        ICACHE.lock().init();
    }

    fn dir_lookup(dev: u32, dir: &InodeBody, name: &[u8]) -> Option<(InodeRef, usize)> {
        if dir.type_ != FileType::Directory {
            panic!("not directory");
        }
        const SZ: usize = core::mem::size_of::<DirEnt>();
        let mut de = core::mem::MaybeUninit::zeroed();
        let mut off = 0;
        while off < dir.size {
            {
                let de_buf =
                    unsafe { core::slice::from_raw_parts_mut(de.as_mut_ptr() as *mut u8, SZ) };
                if dir.read(de_buf, off).unwrap() != SZ {
                    panic!("dir_loolup: read");
                }
            }
            let de: &DirEnt = unsafe { &*de.as_ptr() };
            if de.inum != 0 {
                if name == de.name {
                    return Some((ICACHE.lock().get(dev, de.inum as u32), off));
                }
            }
            off += SZ;
        }
        None
    }

    // Split the path at the end of the first path element.
    // Return a pair of slices.
    // One is a first path element and the other is the remainder.
    // The returned path has no leading slashes,
    // so the caller can check path.is_empty() to see if the name is the last one.
    // If no name to remove, return None.
    //
    // Examples:
    //   split_first("a/bb/c", name) = Some(("a", "bb/c"))
    //   split_first("///a//bb", name) = Some(("a", "bb"))
    //   split_first("a", name) = Some(("a", ""))
    //   split_first("", name) = split_first("////", name) = None
    //
    fn split_first(path: &[u8]) -> Option<(&[u8], &[u8])> {
        fn skip_leading_slash(mut path: &[u8]) -> &[u8] {
            while !path.is_empty() && path[0] == b'/' {
                path = &path[1..];
            }
            path
        }
        let mut path = skip_leading_slash(path);
        if path.is_empty() {
            return None;
        }

        let s = path;
        let mut len = 0;
        while !path.is_empty() && path[0] != b'/' {
            path = &path[1..];
            len += 1;
        }
        let first_elem = &s[..len];
        Some((first_elem, skip_leading_slash(path)))
    }

    fn name_x(path: &str, name_iparent: bool) -> Option<InodeRef> {
        let mut ip = match path {
            "/" => ICACHE.lock().get(ROOT_DEV, ROOT_INO),
            _ =>
            // start traverse from the current working directory
            unsafe { (*my_proc()).cwd.as_ref().unwrap().dup() }
        };

        let mut path = path.as_bytes();
        while let Some((name, path_)) = split_first(path) {
            path = path_;

            let body = ip.body.lock();
            if body.type_ != FileType::Directory {
                return None;
            }

            if name_iparent && path.is_empty() {
                drop(body);
                return Some(ip);
            }

            let (next, _) = dir_lookup(ip.dev, &body, name)?;
            drop(body);
            ip = next;
        }

        if name_iparent {
            None
        } else {
            Some(ip)
        }
    }
    pub fn from_name(path: &str) -> Option<InodeRef> {
        name_x(path, false)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test_case]
        fn test_split_first() {
            let path = b"/foo";
            let (a, b) = split_first(path).unwrap();
            assert_eq!(a, b"foo");
            assert_eq!(b, b"");

            let path = b"a/bb/c";
            let (a, b) = split_first(path).unwrap();
            assert_eq!(a, b"a");
            assert_eq!(b, b"bb/c");

            let path = b"///a//bb";
            let (a, b) = split_first(path).unwrap();
            assert_eq!(a, b"a");
            assert_eq!(b, b"bb");

            let path = b"";
            assert!(split_first(path).is_none());
            let path = b"////";
            assert!(split_first(path).is_none());
        }
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidArg(&'static str),
}
pub type Result<T> = core::result::Result<T, Error>;

pub mod file {
    pub const N_DEV: usize = 10;

    /// table mapping major device number to device functions
    pub struct Dev {
        pub read: Option<fn(&mut [u8]) -> super::Result<usize>>,
        pub write: Option<fn(&[u8]) -> super::Result<usize>>,
    }
    static mut DEV: [Dev; N_DEV] = [Dev {
        read: None,
        write: None,
    }; N_DEV];

    /// Safe wrapper for DEV
    pub fn dev() -> &'static [Dev] {
        unsafe { &DEV[..] }
    }

    pub unsafe fn init_dev(dev_num: u32, dev: Dev) {
        DEV[dev_num as usize] = dev;
    }

    pub const CONSOLE: u32 = 1;
}

pub fn init() {
    bcache::init();
    inode::init();
}
