use utils::prelude::*;

// Initialization happens in two phases.
// 1. main() calls kalloc::init1() while still using entry_page_dir to place just
//      the pages mapped by entry_page_dir on free list.
// 2. main() calls kalloc::init2() with the rest of the physical pages
//      after installing a full page table that maps them on all cores.
pub fn init1() {
    log!("kalloc::init1");
    todo!()
}
