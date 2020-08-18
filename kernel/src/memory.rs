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

pub mod seg {
    // various segment selectors.
    /// kernel code
    pub const SEG_KCODE: usize = 1;
    /// kernel data+stack
    pub const SEG_KDATA: usize = 2;
    /// user code
    pub const SEG_UCODE: usize = 3;
    /// user data+stack
    pub const SEG_UDATA: usize = 4;
    /// this process's task state
    pub const SEG_TSS: usize = 5;
    /// CPU.gdt[SegDesc; NSEGS]; holds the above segments
    pub const NSEGS: usize = 6;

    /// GDT initial value
    pub const GDT_ZERO: [SegDesc; NSEGS] = [
        SegDesc::new(),
        SegDesc::new(),
        SegDesc::new(),
        SegDesc::new(),
        SegDesc::new(),
        SegDesc::new(),
    ];

    pub mod seg_type {
        // Application segment type bits

        /// Executable segment
        pub const STA_X: u8 = 0x8;
        /// Writeable (non-executable segments)
        pub const STA_W: u8 = 0x2;
        /// Readable (executable segments)
        pub const STA_R: u8 = 0x2;

        // System segment type bits

        /// Available 32-bit TSS
        pub const STS_T32A: u8 = 0x9;
        /// 32-bit Interrupt Gate
        pub const STS_IG32: u8 = 0xE;
        /// 32-bit Trap Gate
        pub const STS_TG32: u8 = 0xF;
    }

    pub mod dpl {
        /// Ring-3 (User DPL)
        pub const USER: u8 = 0x3;
    }

    #[derive(Debug, Copy, Clone)]
    #[repr(C)]
    pub struct SegDesc {
        f1: u32,
        f2: u32,
    }
    impl SegDesc {
        #[inline]
        pub const fn new() -> Self {
            Self { f1: 0, f2: 0 }
        }
        #[inline]
        pub const fn from_raw(f1: u32, f2: u32) -> Self {
            Self { f1, f2 }
        }
        #[inline]
        pub const fn set_lim(mut self, limit: u32) -> Self {
            let lim_00_15 = limit & 0x0000FFFF;
            let lim_16_19 = limit & 0x000F0000;
            self.f1 = (self.f1 & 0xFFFF0000) | lim_00_15;
            self.f2 = (self.f2 & 0xFFF0FFFF) | lim_16_19;
            self
        }
        #[inline]
        pub const fn set_base(mut self, base: u32) -> Self {
            let base_00_15 = base & 0x0000FFFF;
            let base_16_23 = base & 0x00FF0000;
            let base_24_31 = base & 0xFF000000;
            self.f1 = (self.f1 & 0x0000FFFF) | (base_00_15 << 16);
            self.f2 = (self.f2 & 0x00FFFF00) | base_24_31 | (base_16_23 >> 16);
            self
        }
        #[inline]
        pub const fn set_flags(mut self, flags: u8) -> Self {
            let flags = (flags & 0xF0) as u32; // [gr, sz, 0, 0]
            self.f2 = (self.f2 & 0xFF0FFFFF) | (flags << 20);
            self
        }
        #[inline]
        pub const fn set_access_byte(mut self, access: u8) -> Self {
            self.f2 = (self.f2 & 0xFFFF00FF) | ((access as u32) << 8);
            self
        }

        #[inline]
        pub const fn seg(ty: u8, base: u32, lim: u32, dpl: u8) -> Self {
            Self::new()
                .set_base(base)
                .set_lim(lim)
                .set_access_byte(0b10010000 | ((dpl & 0b11) << 5) | ty)
                .set_flags(0b1100)
        }
    }
}

pub mod gate {
    #[derive(Copy, Clone)]
    pub struct GateDesc {
        f1: u32,
        f2: u32,
    }
    impl GateDesc {
        pub const fn new() -> Self {
            Self { f1: 0, f2: 0 }
        }
        pub fn set_offset(mut self, offset: u32) -> Self {
            let offset_00_15 = offset & 0x0000FFFF;
            let offset_16_31 = offset & 0xFFFF0000;
            self.f1 = (self.f1 & 0xFFFF0000) | offset_00_15;
            self.f2 = (self.f2 & 0x0000FFFF) | offset_16_31;
            self
        }
        pub fn set_selector(mut self, selector: u16) -> Self {
            self.f1 = (self.f1 & 0x0000FFFF) | (selector as u32);
            self
        }
        pub fn set_type_attribute(mut self, type_attr: u8) -> Self {
            self.f2 = (self.f2 & 0xFFFF00FF) | ((type_attr as u32) << 8);
            self
        }
        /// Set up a normal interrupt/trap gate descriptor.
        /// - is_trap: true for a trap (= exception) gate, false for an interrupt gate.
        ///   interrupt gate clears FL_IF, trap gate leaves FL_IF alone
        /// - selector: Code segment selector for interrupt/trap handler
        /// - offset: Offset in code segment for interrupt/trap handler
        /// - dpl: Descriptor Privilege Level -
        ///        the privilege level required for software to invoke
        ///        this interrupt/trap gate explicitly using an int instruction.
        pub fn set(&mut self, is_trap: bool, selector: u16, offset: u32, dpl: u8) {
            use super::seg::seg_type::{STS_IG32, STS_TG32};

            let present = 1;
            let dpl = dpl & 0x03;
            let s = 0; // Storage Segment : Set to 0 for interrupt and trap gates
            let ty = (if is_trap { STS_TG32 } else { STS_IG32 }) & 0x0F;
            let attr_ty = (present << 7) | (dpl << 5) | (s << 4) | ty;

            //   7                           0
            // +---+---+---+---+---+---+---+---+
            // | P |  DPL  | S |    GateType   |
            // +---+---+---+---+---+---+---+---+
            *self = Self::new()
                .set_offset(offset)
                .set_selector(selector)
                .set_type_attribute(attr_ty);
        }
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
