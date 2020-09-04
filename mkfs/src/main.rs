use std::fs::File;
use std::io::prelude::*;
use std::io::{Result, SeekFrom};
use std::mem::{size_of, transmute};
use std::path::Path;

mod fs;
use fs::*;

const N_BITMAP: usize = FS_SIZE / (BLK_SIZE * 8) + 1;
const N_INODES: usize = 200;
const N_INODE_BLOCKS: usize = N_INODES / INODES_PER_BLOCK + 1;
const N_LOG: usize = LOG_SIZE;

/// Number of meta blocks (boot, sb, log, inode, bitmap)
const N_META: usize = 2 + N_LOG + N_INODE_BLOCKS + N_BITMAP;
/// Number of data blocks
const N_BLOCKS: usize = FS_SIZE - N_META;

const ZEROS: [u8; BLK_SIZE] = [0; BLK_SIZE];

struct FsBuilder {
    pub file: File,
    pub super_block: SuperBlock,
    pub free_inode: InodeNum,
    pub free_block: usize,
}
impl FsBuilder {
    pub fn create<P: AsRef<Path>>(output_file: P, sb: SuperBlock) -> Result<Self> {
        let mut builder = Self {
            file: std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(output_file)?,
            super_block: sb,
            free_inode: 1,
            free_block: N_META,
        };

        // clear all
        for i in 0..FS_SIZE {
            builder.write_sect(i, &ZEROS)?;
        }

        // write the super block
        let mut buf = [0u8; BLK_SIZE];
        unsafe {
            std::ptr::copy_nonoverlapping(&builder.super_block, buf.as_mut_ptr() as *mut _, 1)
        };
        builder.write_sect(1, &buf)?;

        Ok(builder)
    }

    fn set_sect<S: Seek>(s: &mut S, sec: usize) -> Result<&mut S> {
        s.seek(SeekFrom::Start((sec as u64) * (BLK_SIZE as u64)))?;
        Ok(s)
    }

    fn write_sect(&mut self, sec: usize, buf: &Sector) -> Result<()> {
        Self::set_sect(&mut self.file, sec)?.write_all(buf)
    }
    fn read_sect(&mut self, sec: usize, buf: &mut Sector) -> Result<()> {
        Self::set_sect(&mut self.file, sec)?.read_exact(&mut buf[..])
    }

    fn write_inode(&mut self, inum: InodeNum, ind: &OnDiskInode) -> Result<InodeNum> {
        let (bn, idx) = inode_pos(inum, &self.super_block);
        let mut sect = [0u8; BLK_SIZE];
        self.read_sect(bn, &mut sect)?;
        {
            let inodes: &mut InodesSector = unsafe { transmute(&mut sect) };
            inodes[idx] = ind.clone();
        }
        self.write_sect(bn, &sect)?;
        Ok(inum)
    }
    fn read_inode(&mut self, inum: InodeNum, out: &mut OnDiskInode) -> Result<InodeNum> {
        let (bn, idx) = inode_pos(inum, &self.super_block);
        let mut sect = [0u8; BLK_SIZE];
        self.read_sect(bn, &mut sect)?;
        {
            let inodes: &mut InodesSector = unsafe { transmute(&mut sect) };
            *out = inodes[idx].clone();
        }
        Ok(inum)
    }

    fn alloc_block(&mut self) -> Result<()> {
        let used = self.free_block;
        println!("alloc_block: first {} blocks have been allocated", used);
        assert!(used < BLK_SIZE * 8);

        let mut buf = [0u8; BLK_SIZE];
        for i in 0..used {
            buf[i / 8] |= 0x1 << (i % 8);
        }
        println!(
            "alloc_block: write bitmap block at sector {}",
            self.super_block.bmap_start
        );
        self.write_sect(self.super_block.bmap_start as usize, &buf)
    }
    fn alloc_inode(&mut self, ty: FileType) -> Result<InodeNum> {
        let inum = self.free_inode;
        self.free_inode += 1;

        let mut din = OnDiskInode::default();
        din.type_ = (ty as u16).to_le();
        din.n_link = 1u16.to_le();
        din.size = 0u32.to_le();
        self.write_inode(inum, &din)
    }

    fn take_next_block(&mut self) -> usize {
        let r = self.free_block;
        self.free_block += 1;
        r
    }
    fn append_inode(&mut self, inum: InodeNum, mut data: &[u8]) -> Result<()> {
        let mut din = OnDiskInode::default();
        self.read_inode(inum, &mut din)?;

        let mut off = din.size.to_le() as usize;
        while !data.is_empty() {
            let fbn = off / BLK_SIZE;
            assert!(fbn < MAX_FILE);
            let sect = if fbn < N_DIRECT {
                if din.addrs[fbn].to_le() == 0 {
                    din.addrs[fbn] = self.take_next_block() as u32;
                }
                din.addrs[fbn].to_le() as usize
            } else {
                if din.addrs[N_DIRECT].to_le() == 0 {
                    din.addrs[N_DIRECT] = self.take_next_block() as u32;
                }
                let mut indirect: [u32; N_INDIRECT] = [0u32; N_INDIRECT];
                {
                    assert_eq!(size_of::<Sector>(), size_of::<[u32; N_INDIRECT]>());
                    let indirect: &mut Sector = unsafe { transmute(&mut indirect) };
                    self.read_sect(din.addrs[N_DIRECT].to_le() as usize, indirect)?;
                }
                if indirect[fbn - N_DIRECT] == 0 {
                    indirect[fbn - N_DIRECT] = self.take_next_block() as u32;

                    let indirect: &Sector = unsafe { transmute(&indirect) };
                    self.write_sect(din.addrs[N_DIRECT].to_le() as usize, indirect)?;
                }
                indirect[fbn - N_DIRECT].to_le() as usize
            };

            let nw = usize::min(data.len(), (fbn + 1) * BLK_SIZE - off);
            let mut buf = [0u8; BLK_SIZE];
            self.read_sect(sect, &mut buf)?;
            let begin = off % BLK_SIZE;
            buf[begin..begin + nw].copy_from_slice(&data[..nw]);
            self.write_sect(sect, &buf)?;

            off += nw;
            data = &data[nw..];
        }

        din.size = off.to_le() as u32;
        self.write_inode(inum, &din)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    assert!(BLK_SIZE % size_of::<OnDiskInode>() == 0);
    assert!(BLK_SIZE % size_of::<DirEnt>() == 0);

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mkfs [output-image] [files...]");
        return Ok(());
    }

    let mut builder = {
        let mut sb = SuperBlock::default();
        sb.size = (FS_SIZE as u32).to_le();
        sb.n_blocks = (N_BLOCKS as u32).to_le();
        sb.n_inodes = (N_INODES as u32).to_le();
        sb.n_log = (N_LOG as u32).to_le();
        sb.log_start = 2u32.to_le();
        sb.inode_start = (2 + N_LOG as u32).to_le();
        sb.bmap_start = (2 + N_LOG as u32 + N_INODE_BLOCKS as u32).to_le();
        FsBuilder::create(&args[1], sb)?
    };

    println!("nmeta {} (boot, super, log blocks {}, inode blocks {}, bitmap blocks {}) blocks {} total {}", N_META, N_LOG, N_INODE_BLOCKS, N_BITMAP, N_BLOCKS, FS_SIZE);
    assert_eq!(N_META + N_BLOCKS, FS_SIZE);

    let root_ino = builder.alloc_inode(FileType::Directory)?;
    assert_eq!(root_ino, ROOT_INO);
    {
        let mut de = DirEnt::default();
        de.inum = root_ino.to_le();
        de.set_name(b".");
        builder.append_inode(root_ino, de.as_bytes())?;

        let mut de = DirEnt::default();
        de.inum = root_ino.to_le();
        de.set_name(b"..");
        builder.append_inode(root_ino, de.as_bytes())?;
    }

    for filename in &args[2..] {
        let mut file = File::open(filename)?;
        let filename: &Path = filename.as_ref();
        let mut filename = filename.file_name().unwrap().to_str().unwrap().as_bytes();

        // Skip leading _ in name when writing to file system.
        // The binaries are named _rm, _cat, etc. to keep the
        // build operating system from trying to execute them
        // in place of system binaries like rm and cat.
        if filename.starts_with(b"_") {
            filename = &filename[1..];
        }

        let inum = builder.alloc_inode(FileType::Regular)?;

        let mut de = DirEnt::default();
        de.inum = inum.to_le();
        de.set_name(filename);
        builder.append_inode(root_ino, de.as_bytes())?;

        let mut buf = [0u8; BLK_SIZE];
        loop {
            let nread = file.read(&mut buf)?;
            if nread == 0 {
                // EOF
                break;
            }
            builder.append_inode(inum, &buf[..nread])?;
        }
    }

    // fix size of root inode dir
    let mut din = OnDiskInode::default();
    builder.read_inode(root_ino, &mut din)?;
    let off = din.size.to_le() as usize;
    let off = ((off / BLK_SIZE) + 1) * BLK_SIZE;
    din.size = (off as u32).to_le();
    builder.write_inode(root_ino, &din)?;

    // write the bitmap block
    builder.alloc_block()?;

    Ok(())
}
