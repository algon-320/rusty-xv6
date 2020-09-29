const BLK_SIZE: usize = 512;

pub mod ide {
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
}

pub mod bcache {
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
        pub body: SleepMutex<BufBody>,
    }
    pub struct BufBody {
        pub data: [u8; BLK_SIZE],
    }
    impl Buf {
        pub const fn zero() -> Self {
            Self {
                dev: 0,
                block_no: 0,
                flags: AtomicU8::new(0),
                body: SleepMutex::new(
                    "buf",
                    BufBody {
                        data: [0; BLK_SIZE],
                    },
                ),
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
    use super::{Error, Result};
    use crate::lock::sleep::SleepMutex;
    use crate::lock::spin::SpinMutex;
    use crate::proc::my_proc;
    use alloc::collections::BTreeMap;
    use alloc::sync::{Arc, Weak};
    use lazy_static::lazy_static;

    const ROOT_DEV: u32 = 1;
    const ROOT_INO: u32 = 1;

    pub type InodeRef = Arc<Inode>;

    /// in-memory copy of an inode
    pub struct Inode {
        dev: u32,  // Device number
        inum: u32, // Inode number
        body: SleepMutex<InodeBody>,
    }
    impl Inode {
        pub const fn zero() -> Self {
            Self {
                dev: 0,
                inum: 0,
                body: SleepMutex::new("inode", InodeBody::zero()),
            }
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

    impl Drop for Inode {
        fn drop(&mut self) {
            log!("Inode drop");
            let mut body = self.body.lock();

            if body.valid && body.nlink == 0 {
                // inode has no links and no other references: truncate and free.
                // TODO: trunc
                body.type_ = FileType::Invalid;
                // TODO: update
                body.valid = false;
                todo!();
            }
        }
    }

    pub struct Icache {
        cache: BTreeMap<(u32, u32), Weak<Inode>>,
    }
    impl Icache {
        pub fn new() -> Self {
            Self {
                cache: BTreeMap::new(),
            }
        }
        pub fn get(&mut self, dev: u32, inum: u32) -> InodeRef {
            let key = (dev, inum);
            match self.cache.get(&key).and_then(|weak| weak.upgrade()) {
                Some(arc) => arc,
                None => {
                    let mut inode = Arc::new(Inode::zero());
                    {
                        let inode = Arc::get_mut(&mut inode).unwrap();
                        inode.dev = dev;
                        inode.inum = inum;
                    }
                    let weak = Arc::downgrade(&inode);
                    self.cache.insert(key, weak);
                    inode
                }
            }
        }
    }

    lazy_static! {
        static ref ICACHE: SpinMutex<Icache> = SpinMutex::new("icache", Icache::new());
    }

    pub fn init() {
        lazy_static::initialize(&ICACHE);
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
            _ => {
                // start traverse from the current working directory
                my_proc().lock().cwd.as_ref().unwrap().clone()
            }
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
    ide::init();
    bcache::init();
    inode::init();
}
