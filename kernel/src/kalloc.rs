use super::memory;
use super::memory::pg_dir::PAGE_SIZE;
use super::memory::v2p;
use super::spinlock::SpinLock;
use utils::prelude::*;

type Page = [u8; PAGE_SIZE];

#[repr(C)]
struct Run {
    pub next: *mut Run,
}

pub struct Kmem {
    lock: SpinLock,
    use_lock: bool,
    free_list: *mut Run,
}

extern "C" {
    ///  first address after kernel loaded from ELF file
    static kernel_end: core::ffi::c_void;
}

impl Kmem {
    /// Initialization happens in two phases.
    /// 1. main() calls kalloc::init1() while still using entry_page_dir to place just
    ///      the pages mapped by entry_page_dir on free list.
    /// 2. main() calls kalloc::init2() with the rest of the physical pages
    ///      after installing a full page table that maps them on all cores.
    pub fn init1(start: VAddr<u8>, end: VAddr<u8>) -> Self {
        let mut ctx = Self {
            lock: SpinLock::new("kmem"),
            use_lock: false,
            free_list: core::ptr::null_mut(),
        };
        ctx.free_range(start, end);
        ctx
    }

    fn free_range(&mut self, start: VAddr<u8>, end: VAddr<u8>) {
        log!("free_range: start:{:p}, end:{:p}", start.ptr(), end.ptr());
        let page = start.round_up(PAGE_SIZE);
        let mut page = page.cast::<Page>();
        let mut avail_page = 0;
        while (page + 1).cast() < end {
            self.kfree(page);
            page += 1;
            avail_page += 1;
        }
        log!("{} pages available", avail_page);
    }

    /// Free the page of physical memory pointed at by page,
    /// which normally should have been returned by a call to kalloc().
    /// (The exception is when initializing the allocator; see init above.)
    pub fn kfree(&mut self, page: VAddr<Page>) {
        let end = unsafe { &kernel_end as *const _ as usize };
        assert!(page.raw() >= end);
        assert!(v2p(page).raw() < memory::PHYSTOP);

        // Fill with junk to catch dangling refs
        unsafe { rlibc::memset(page.cast().mut_ptr(), 1, PAGE_SIZE) };

        if self.use_lock {
            self.lock.acquire();
        }
        let r: *mut Run = page.cast().mut_ptr();
        unsafe { (*r).next = self.free_list };
        self.free_list = r;
        if self.use_lock {
            self.lock.release();
        }
    }

    /// Allocate one 4096-byte page of physical memory.
    /// Returns a pointer that the kernel can use.
    /// Returns 0 if the memory cannot be allocated.
    pub fn kalloc(&mut self) -> VAddr<Page> {
        if self.use_lock {
            self.lock.acquire();
        }
        let r = self.free_list;
        if !r.is_null() {
            self.free_list = unsafe { (*r).next };
        }
        if self.use_lock {
            self.lock.release();
        }
        VAddr::from(r as *mut _)
    }
}
