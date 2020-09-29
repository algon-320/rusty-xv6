pub mod bcache;
pub mod ide;
pub mod inode;

const BLK_SIZE: usize = 512;
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
