#![allow(dead_code)]

use std::mem::size_of;

pub type InodeNum = u16;
pub type Sector = [u8; BLK_SIZE];
pub type InodesSector = [OnDiskInode; INODES_PER_BLOCK];

/// root i-number
pub const ROOT_INO: InodeNum = 1;
/// block size
pub const BLK_SIZE: usize = 512;

pub const N_DIRECT: usize = 12;
pub const N_INDIRECT: usize = BLK_SIZE / size_of::<u32>();
pub const MAX_FILE: usize = N_DIRECT + N_INDIRECT;
pub const DIR_SIZE: usize = 14;

/// Max # of blocks any FS op writes
pub const MAX_OP_BLOCKS: usize = 10;
/// Max data blocks in on-disk log
pub const LOG_SIZE: usize = MAX_OP_BLOCKS * 3;
/// Size of file system (blocks)
pub const FS_SIZE: usize = 1000;

/// Inodes per block
pub const INODES_PER_BLOCK: usize = BLK_SIZE / size_of::<OnDiskInode>();

/// Disk layout:
/// [ boot block | super block | log | inode blocks | free bit map | data blocks ]
#[derive(Default)]
#[repr(C)]
pub struct SuperBlock {
    /// Size of file system image (blocks)
    pub size: u32,
    /// Number of data blocks
    pub n_blocks: u32,
    /// Number of inodes
    pub n_inodes: u32,
    /// Number of log blocks
    pub n_log: u32,
    /// Block number of first log block
    pub log_start: u32,
    /// Block number of first inode block
    pub inode_start: u32,
    /// Block number of fisrt free map blocks
    pub bmap_start: u32,
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct OnDiskInode {
    /// File type
    pub type_: u16,
    /// Major device number
    pub major: u16,
    /// Minor device number
    pub minor: u16,
    /// Number of links to inode in file system
    pub n_link: u16,
    /// Size of file (bytes)
    pub size: u32,
    /// Data block addresses
    pub addrs: [u32; N_DIRECT + 1],
}

#[derive(Default)]
#[repr(C)]
pub struct DirEnt {
    pub inum: InodeNum,
    pub name: [u8; DIR_SIZE],
}
impl DirEnt {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self as *const _ as *const u8, size_of::<Self>()) }
    }
    pub fn set_name(&mut self, name: &[u8]) {
        assert!(name.iter().copied().find(|c| *c == b'/').is_none());
        self.name[..name.len()].copy_from_slice(name);
        let z = usize::min(name.len() + 1, DIR_SIZE);
        self.name[z - 1] = 0; // null terminate
    }
}

#[repr(u16)]
pub enum FileType {
    Directory = 1,
    Regular = 2,
    Device = 3,
}

/// (block, index) containing inode i
pub fn inode_pos(i: InodeNum, sb: &SuperBlock) -> (usize, usize) {
    let blk = (sb.inode_start as usize) + (i as usize) / INODES_PER_BLOCK;
    let idx = (i as usize) % INODES_PER_BLOCK;
    (blk, idx)
}
