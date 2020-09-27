use super::lock::spin::SpinMutex;
use super::memory::{Page, PAGE_SIZE};
use alloc::alloc::{alloc, dealloc, GlobalAlloc, Layout};
use core::ptr::NonNull;
use utils::prelude::*;

use linked_list_allocator::Heap;
pub struct KernelHeap {
    heap: SpinMutex<Heap>,
}
unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.heap
            .lock()
            .allocate_first_fit(layout)
            .unwrap()
            .as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug_assert!(!ptr.is_null());
        self.heap
            .lock()
            .deallocate(NonNull::new_unchecked(ptr), layout);
    }
}
impl KernelHeap {
    pub unsafe fn init(&self, start: usize, size: usize) {
        self.heap.lock().init(start, size);
    }
    pub unsafe fn extend(&self, size: usize) {
        self.heap.lock().extend(size);
    }
}
#[global_allocator]
static HEAP: KernelHeap = KernelHeap {
    heap: SpinMutex::new("kheap", Heap::empty()),
};

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

/// Initialization happens in two phases.
/// 1. main() calls kalloc::init1() while still using entry_page_dir to place just
///      the pages mapped by entry_page_dir on free list.
/// 2. main() calls kalloc::init2() with the rest of the physical pages
///      after installing a full page table that maps them on all cores.
pub fn init1(start: VAddr<u8>, end: VAddr<u8>) {
    let size = end.raw() - start.raw();
    unsafe { HEAP.init(start.raw(), size) };
}
pub fn init2(start: VAddr<u8>, end: VAddr<u8>) {
    let size = end.raw() - start.raw();
    unsafe { HEAP.extend(size) };
}

/// Free the page of physical memory pointed at by page,
/// which normally should have been returned by a call to kalloc().
pub fn kfree(page: NonNull<Page>) {
    let layout = Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap();
    unsafe { dealloc(page.as_ptr() as *mut u8, layout) };
}

/// Allocate one 4096-byte page of physical memory.
/// Returns a pointer that the kernel can use.
/// Returns None if the memory cannot be allocated.
pub fn kalloc() -> Option<NonNull<Page>> {
    let layout = Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap();
    let page = unsafe { alloc(layout) };
    NonNull::new(page as *mut Page)
}
