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

pub mod inode {
    use super::file;
    use super::DirEnt;
    use super::{Error, Result};
    use crate::lock::sleep::SleepMutex;
    use crate::lock::spin::SpinMutex;
    use crate::proc::my_proc;
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicUsize, Ordering};

    const ROOT_DEV: usize = 1;
    const ROOT_INO: usize = 1;

    /// maximum number of active i-nodes
    const N_INODE: usize = 50;

    static ICACHE: SpinMutex<[Inode; N_INODE]> = SpinMutex::new("icache", [Inode::zero(); N_INODE]);

    /// in-memory copy of an inode
    pub struct Inode {
        dev: usize,                 // Device number
        inum: usize,                // Inode number
        ref_cnt: UnsafeCell<usize>, // Reference count
        body: SleepMutex<InodeBody>,
    }
    impl Inode {
        pub const fn zero() -> Self {
            Self {
                dev: 0,
                inum: 0,
                ref_cnt: UnsafeCell::new(0),
                body: SleepMutex::new("inode", InodeBody::zero()),
            }
        }
        /// Safety: ICACHE must be locked.
        unsafe fn increment_ref_cnt(&self) {
            *self.ref_cnt.get() += 1;
        }
        /// Safety: ICACHE must be locked.
        unsafe fn get_ref_cnt(&self) -> usize {
            *self.ref_cnt.get()
        }
        /// Safety: ICACHE must be locked.
        unsafe fn decrement_ref_cnt(&self) {
            *self.ref_cnt.get() += 1;
        }
        /// Safety: ICACHE must be locked.
        pub unsafe fn new_ref(&self) -> InodeRef {
            self.increment_ref_cnt();
            InodeRef::new(self)
        }

        fn trunc(&self) {
            let mut body = self.body.lock();
            for addr in body.addrs[..super::N_DIRECT].iter_mut() {
                if *addr != 0 {
                    todo!(); // bfree
                    *addr = 0;
                }
            }
        }
    }
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

    pub struct InodeRef {
        ptr: core::ptr::NonNull<Inode>, // use as non-null *const Inode
    }
    impl InodeRef {
        pub fn new(ip: &Inode) -> Self {
            Self {
                ptr: unsafe { core::ptr::NonNull::new_unchecked(ip as *const _ as *mut _) },
            }
        }
    }
    impl core::clone::Clone for InodeRef {
        fn clone(&self) -> Self {
            let _ = ICACHE.lock();
            unsafe { self.new_ref() }
        }
    }
    impl Drop for InodeRef {
        fn drop(&mut self) {
            log!("InodeRef drop");
            let mut body = self.body.lock();

            if body.valid && body.nlink == 0 {
                let ref_cnt = {
                    let _guard = ICACHE.lock();
                    unsafe { self.get_ref_cnt() }
                };
                if ref_cnt == 1 {
                    // inode has no links and no other references: truncate and free.
                    // TODO: trunc
                    body.type_ = FileType::Invalid;
                    // TODO: update
                    body.valid = false;
                    todo!()
                }
            }
        }
    }
    impl core::ops::Deref for InodeRef {
        type Target = Inode;
        fn deref(&self) -> &Self::Target {
            unsafe { &*self.ptr.as_ptr() }
        }
    }

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

    fn init() {
        todo!()
    }

    fn get(dev: usize, inum: usize) -> InodeRef {
        let mut icache = ICACHE.lock();

        let mut empty: *mut Inode = core::ptr::null_mut();
        for ip in icache.iter_mut() {
            let ref_cnt = unsafe { ip.get_ref_cnt() };
            if ref_cnt > 0 && ip.dev == dev && ip.inum == inum {
                return unsafe { ip.new_ref() }; // ref_cnt += 1
            }
            if empty.is_null() && ref_cnt == 0 {
                empty = ip;
            }
        }
        assert!(!empty.is_null(), "inode::get: no inodes");

        let ip = unsafe { &mut *empty };
        ip.dev = dev;
        ip.inum = inum;
        ip.ref_cnt = UnsafeCell::new(0);
        ip.body.lock().valid = false;
        unsafe { ip.new_ref() } // ref_cnt -> 1
    }

    fn dir_lookup(dev: usize, dir: &InodeBody, name: &[u8]) -> Option<(InodeRef, usize)> {
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
                    return Some((get(dev, de.inum as usize), off));
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
            "/" => get(ROOT_DEV, ROOT_INO),
            _ =>
            // start traverse from the current working directory
            unsafe { (*my_proc()).cwd.as_ref().unwrap().clone() }
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

    pub unsafe fn init_dev(dev_num: usize, dev: Dev) {
        DEV[dev_num] = dev;
    }

    pub const CONSOLE: usize = 1;
}
