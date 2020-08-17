use super::memory;
use super::memory::{v2p, Page, PAGE_SIZE};
use super::spinlock::SpinLock;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};
use utils::prelude::*;

#[repr(transparent)]
struct Run {
    pub next: *mut Run,
}
struct Kmem {
    use_lock: AtomicBool,
    lock: SpinLock,
    free_list: UnsafeCell<*mut Run>,
}
impl Kmem {
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: Fn(&mut *mut Run) -> R,
    {
        if self.use_lock.load(Ordering::SeqCst) {
            self.lock.acquire();
            let r = unsafe { f(&mut *self.free_list.get()) };
            self.lock.release();
            r
        } else {
            unsafe { f(&mut *self.free_list.get()) }
        }
    }
    pub fn lock(&self) {
        self.use_lock.store(true, Ordering::SeqCst);
    }
}
unsafe impl Sync for Kmem {}

static KMEM: Kmem = Kmem {
    use_lock: AtomicBool::new(false),
    lock: SpinLock::new("kmem"),
    free_list: UnsafeCell::new(core::ptr::null_mut()),
};

/// Initialization happens in two phases.
/// 1. main() calls kalloc::init1() while still using entry_page_dir to place just
///      the pages mapped by entry_page_dir on free list.
/// 2. main() calls kalloc::init2() with the rest of the physical pages
///      after installing a full page table that maps them on all cores.
pub fn init1(start: VAddr<u8>, end: VAddr<u8>) {
    free_range(start, end);
}
pub fn init2(start: VAddr<u8>, end: VAddr<u8>) {
    free_range(start, end);
    KMEM.lock();
}

fn free_range(start: VAddr<u8>, end: VAddr<u8>) {
    log!("free_range: start:{:p}, end:{:p}", start.ptr(), end.ptr());
    let page = start.round_up(PAGE_SIZE);
    let mut page = page.cast::<Page>();
    let mut avail_page = 0;
    while (page + 1).cast() <= end {
        kfree(page.mut_ptr());
        page += 1;
        avail_page += 1;
    }
    log!("{} pages available", avail_page);
}

/// Free the page of physical memory pointed at by page,
/// which normally should have been returned by a call to kalloc().
/// (The exception is when initializing the allocator; see init above.)
pub fn kfree(page: *mut Page) {
    extern "C" {
        ///  first address after kernel loaded from ELF file
        static kernel_end: core::ffi::c_void;
    }
    let end = unsafe { &kernel_end as *const _ as usize };
    assert!(page as usize >= end);
    assert!(v2p(VAddr::from(page)) < memory::PHYSTOP);

    // Fill with junk to catch dangling refs
    unsafe { rlibc::memset(page as *mut u8, 1, PAGE_SIZE) };

    KMEM.with(|free_list| {
        let r = page as *mut Run;
        unsafe { (*r).next = *free_list };
        *free_list = r;
    });
}

/// Allocate one 4096-byte page of physical memory.
/// Returns a pointer that the kernel can use.
/// Returns None if the memory cannot be allocated.
pub fn kalloc() -> Option<*mut Page> {
    KMEM.with(|free_list| {
        let r = *free_list;
        if !r.is_null() {
            unsafe { *free_list = (*r).next };
            Some(r as *mut Page)
        } else {
            log!("kalloc failed: returning null");
            None
        }
    })
}
