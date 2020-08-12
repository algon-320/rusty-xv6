/// x86 page directory
pub mod pg_dir {
    /// # directory entries per page directory
    pub const NPDENTRIES: usize = 1024;

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct PageDirEntry(u32);

    /// Quoted from https://wiki.osdev.org/Paging
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

    impl PageDirEntry {
        /// Creates new entry.
        /// `page_table_addr` must be 4KiB aligned (lower 12 bit must be zero)
        pub const fn table_ref(page_table_addr: u32, flags: u32) -> Self {
            Self(page_table_addr | flags)
        }
        /// Creates new entry (direct address of 4MiB page).
        /// `page_table_addr` must be 4MiB aligned (lower 22 bit must be zero)
        pub const fn large_page(page_addr: u32, flags: u32) -> Self {
            Self(page_addr | ent_flag::PAGE_SIZE_4MIB | flags)
        }
        pub const fn zero() -> Self {
            Self(0)
        }
    }

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct PageTableEntry(u32);
}

/// First kernel virtual address
pub const KERNBASE: usize = 0x80000000;

use utils::prelude::*;
#[inline]
pub fn p2v<T>(pa: PAddr<T>) -> VAddr<T> {
    let raw = pa.raw();
    VAddr::from((raw + KERNBASE) as *mut _)
}
#[inline]
pub fn v2p<T>(pa: VAddr<T>) -> PAddr<T> {
    let raw = pa.raw();
    VAddr::from((raw - KERNBASE) as *mut _)
}
