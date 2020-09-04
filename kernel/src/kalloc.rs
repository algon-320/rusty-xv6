use super::lock::spin::SpinMutex;
use super::memory;
use super::memory::{v2p, Page, PAGE_SIZE};
use utils::prelude::*;

#[repr(transparent)]
struct Run {
    pub next: *mut Run,
}
unsafe impl Send for Run {}
static KMEM: SpinMutex<Run> = SpinMutex::new(
    "kmem",
    Run {
        next: core::ptr::null_mut(),
    },
);

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
}

fn free_range(start: VAddr<u8>, end: VAddr<u8>) {
    log!("free_range: start:{:p}, end:{:p}", start.ptr(), end.ptr());
    let mut page = start.round_up(PAGE_SIZE).cast::<Page>();
    let end = end.round_down(PAGE_SIZE).cast::<Page>();
    let mut avail_page = 0;
    while page < end {
        kfree(page.mut_ptr());
        page += 1;
        avail_page += 1;
    }
    log!("+{} pages", avail_page);
}

/// Free the page of physical memory pointed at by page,
/// which normally should have been returned by a call to kalloc().
/// (The exception is when initializing the allocator; see init above.)
pub fn kfree(page: *mut Page) {
    extern "C" {
        ///  first address after kernel loaded from ELF file
        static kernel_end: u8;
    }
    let end = unsafe { &kernel_end as *const _ as usize };
    assert!(page as usize >= end);
    assert!(v2p(VAddr::from(page)) < memory::PHYSTOP);

    // Fill with junk to catch dangling refs
    unsafe { rlibc::memset(page as *mut u8, 1, PAGE_SIZE) }; // TODO: make this fast

    let mut free_list = KMEM.lock();
    let r = page as *mut Run;
    unsafe { (*r).next = free_list.next };
    free_list.next = r;
}

/// Allocate one 4096-byte page of physical memory.
/// Returns a pointer that the kernel can use.
/// Returns None if the memory cannot be allocated.
pub fn kalloc() -> Option<*mut Page> {
    let mut free_list = KMEM.lock();
    let r = free_list.next;
    if !r.is_null() {
        free_list.next = unsafe { (*r).next };
        Some(r as *mut Page)
    } else {
        log!("kalloc failed: returning None");
        None
    }
}
