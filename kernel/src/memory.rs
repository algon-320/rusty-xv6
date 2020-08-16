/// x86 page directory
///
/// for details:
///     Intel@ 64 and IA-32 Architectures Software Developer's Manual,
///     Vol.3: System Programming Guide - 4.3 (32-bit Paging)
pub mod pg_dir {
    use utils::address::{PAddr, VAddr};

    /// # directory entries per page directory
    pub const NPDENTRIES: usize = 1024;
    /// # PTEs per page table
    pub const NPTENTRIES: usize = 1024;

    pub type PageDirectory = [PageDirEntry; NPDENTRIES];
    pub type PageTable = [PageTableEntry; NPTENTRIES];

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct PageDirEntry(u32);

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct PageTableEntry(u32);

    const DIR_ENT_FLAG_MASK: u32 = 0b111110111111;
    const TAB_ENT_FLAG_MASK: u32 = 0b111101111111;

    // A virtual address 'va' has a three-part structure as follows:
    //
    // +--------10------+-------10-------+---------12----------+
    // | Page Directory |   Page Table   | Offset within Page  |
    // |      Index     |      Index     |                     |
    // +----------------+----------------+---------------------+
    //  \--- PDX(va) --/ \--- PTX(va) --/

    #[inline]
    pub fn pdx<T>(va: VAddr<T>) -> usize {
        (va.raw() >> 22) & 0x3FF
    }
    #[inline]
    pub fn ptx<T>(va: VAddr<T>) -> usize {
        (va.raw() >> 12) & 0x3FF
    }

    impl PageDirEntry {
        /// Creates new entry.
        /// `page_table_addr` must be 4KiB aligned (lower 12 bit must be zero)
        pub const fn new_table(page_table_addr: u32, flags: u32) -> Self {
            Self(page_table_addr | flags & DIR_ENT_FLAG_MASK)
        }
        /// Creates new entry (direct address of 4MiB page).
        /// `page_table_addr` must be 4MiB aligned (lower 22 bit must be zero)
        pub const fn new_large_page(page_addr: PAddr<super::Page>, flags: u32) -> Self {
            Self(page_addr.raw() as u32 | ent_flag::PAGE_SIZE_4MIB | flags & DIR_ENT_FLAG_MASK)
        }
        pub const fn zero() -> Self {
            Self(0)
        }

        #[inline]
        pub fn set_flags(&mut self, flags: u32) {
            self.0 |= flags & DIR_ENT_FLAG_MASK;
        }
        #[inline]
        pub fn addr(self) -> PAddr<PageTable> {
            PAddr::from_raw((self.0 & !DIR_ENT_FLAG_MASK) as usize)
        }
        #[inline]
        pub fn flags(self) -> u32 {
            self.0 & DIR_ENT_FLAG_MASK
        }
        #[inline]
        pub fn flags_check(self, mask: u32) -> bool {
            (self.0 & DIR_ENT_FLAG_MASK & mask) == mask
        }
    }

    impl PageTableEntry {
        #[inline]
        pub fn new(page_addr: PAddr<super::Page>, flags: u32) -> Self {
            Self(page_addr.raw() as u32 | flags & TAB_ENT_FLAG_MASK)
        }
        #[inline]
        pub fn set_flags(&mut self, flags: u32) {
            self.0 |= flags & TAB_ENT_FLAG_MASK;
        }
        #[inline]
        pub fn addr(self) -> PAddr<super::Page> {
            PAddr::from_raw((self.0 & !TAB_ENT_FLAG_MASK) as usize)
        }
        #[inline]
        pub fn flags(self) -> u32 {
            self.0 & TAB_ENT_FLAG_MASK
        }
        #[inline]
        pub fn flags_check(self, mask: u32) -> bool {
            (self.0 & TAB_ENT_FLAG_MASK & mask) == mask
        }
    }

    #[allow(dead_code)]
    pub mod ent_flag {
        /// If the bit is set, then pages are 4 MiB in size. Otherwise, they are 4 KiB.
        /// Please note that 4-MiB pages require PSE to be enabled.
        pub const PAGE_SIZE_4MIB: u32 = 0b000010000000;
        /// If the bit is set, the page will not be cached. Otherwise, it will be.
        pub const CACHE_DISABLE: u32 = 0b000000010000;
        /// If the bit is set, write-through caching is enabled. If not, then write-back is enabled instead.
        pub const WRITE_THROUGH: u32 = 0b000000001000;
        /// If the bit is set, then the page may be accessed by all;
        /// if the bit is not set, however, only the supervisor can access it.
        /// For a page directory entry, the user bit controls access to all the pages referenced by the page directory entry.
        pub const USER: u32 = 0b000000000100;
        /// If the bit is set, the page is read/write. Otherwise when it is not set, the page is read-only.
        pub const WRITABLE: u32 = 0b000000000010;
        /// If the bit is set, the page is actually in physical memory at the moment.
        pub const PRESENT: u32 = 0b000000000001;
    }
}

pub type Page = [u8; PAGE_SIZE];

/// Page size
pub const PAGE_SIZE: usize = 4096;

/// First kernel virtual address
pub const KERNBASE: VAddr<Page> = unsafe { VAddr::from_raw_unchecked(0x80000000) };
/// Address where kernel is linked (KERNBASE + EXTMEM)
pub const KERNLINK: VAddr<Page> = unsafe { VAddr::from_raw_unchecked(0x80100000) };

/// Start of extended memory
pub const EXTMEM: PAddr<Page> = unsafe { PAddr::from_raw_unchecked(0x100000) };
/// Top physical memory
pub const PHYSTOP: PAddr<Page> = unsafe { PAddr::from_raw_unchecked(0xE000000) };
/// Other devices are at high addresses
pub const DEVSPACE: VAddr<Page> = unsafe { VAddr::from_raw_unchecked(0xFE000000) };

use utils::prelude::*;
#[inline]
pub const fn p2v<T>(pa: PAddr<T>) -> VAddr<T> {
    let raw = pa.raw();
    unsafe { VAddr::from_raw_unchecked(raw + KERNBASE.raw()) }
}
#[inline]
pub const fn v2p<T>(pa: VAddr<T>) -> PAddr<T> {
    let raw = pa.raw();
    unsafe { PAddr::from_raw_unchecked(raw - KERNBASE.raw()) }
}
