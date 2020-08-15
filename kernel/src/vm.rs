use super::kalloc;
use super::memory::pg_dir::{
    self, ent_flag, PageDirEntry, PageDirectory, PageTable, PageTableEntry,
};
use super::memory::{p2v, v2p, Page};
use super::memory::{DEVSPACE, EXTMEM, KERNBASE, KERNLINK, PAGE_SIZE, PHYSTOP};
use utils::prelude::*;

struct Kmap {
    virt: VAddr<Page>,
    start: PAddr<Page>,
    end: PAddr<Page>,
    perm: u32,
}

// Return the reference of the PTE in page table pg_dir
// that corresponds to virtual address va.  If alloc!=0,
// create any required page table pages.
fn walk_page_dir(
    pg_dir: &mut PageDirectory,
    va: VAddr<Page>,
    alloc: bool,
) -> Option<&mut PageTableEntry> {
    let pde = &mut pg_dir[pg_dir::pdx(va)];
    let pg_tab = if pde.flags_check(ent_flag::PRESENT) {
        p2v(pde.addr())
    } else {
        if !alloc {
            return None;
        }
        let pg_tab = VAddr::from(kalloc::kalloc()? as *mut PageTable);

        // Make sure all those PTE_P bits are zero.
        unsafe { rlibc::memset(pg_tab.cast().mut_ptr(), 0, PAGE_SIZE) };

        // // The permissions here are overly generous, but they can
        // // be further restricted by the permissions in
        // // the page table entries, if necessary.
        *pde = PageDirEntry::new_table(
            v2p(pg_tab).raw() as u32,
            ent_flag::PRESENT | ent_flag::WRITABLE | ent_flag::USER,
        );
        // assert!(*pde & mmu::PteFlags::PRESENT.bits() != 0);
        pg_tab
    };
    unsafe { Some(&mut (*pg_tab.mut_ptr())[pg_dir::ptx(va)]) }
}

// Create PTEs for virtual addresses starting at va that refer to
// physical addresses starting at pa. va and size might not be page-aligned.
fn map_pages(
    pg_dir: &mut PageDirectory,
    va: VAddr<u8>,
    size: usize,
    mut pa: PAddr<Page>,
    perm: u32,
) -> Option<()> {
    let mut a: VAddr<Page> = va.round_down(PAGE_SIZE).cast();
    let last: VAddr<Page> = (va + size - 1).round_down(PAGE_SIZE).cast();
    let mut cnt = 0;
    loop {
        let pte = walk_page_dir(pg_dir, a, true)?;
        if pte.flags_check(ent_flag::PRESENT) {
            panic!("remap");
        }
        *pte = PageTableEntry::new(pa, perm | ent_flag::PRESENT);
        if a == last {
            break;
        }
        a += 1;
        pa += 1;
        cnt += 1;
    }
    log!("map_pages: cnt = {}", cnt);
    Some(())
}

/// Set up kernel part of a page table.
fn setup_kvm<'kmem>() -> Option<&'kmem mut PageDirectory> {
    let data_vaddr = {
        extern "C" {
            static data: core::ffi::c_void;
        }
        VAddr::from_raw(unsafe { &data } as *const _ as usize)
    };
    let data_paddr = v2p(data_vaddr);
    let kmap = [
        // I/O space
        Kmap {
            virt: KERNBASE,
            start: PAddr::from_raw(0),
            end: EXTMEM,
            perm: ent_flag::WRITABLE,
        },
        // kern text+rodata
        Kmap {
            virt: KERNLINK,
            start: v2p(KERNLINK),
            end: data_paddr,
            perm: 0,
        },
        // kern data+memory
        Kmap {
            virt: data_vaddr,
            start: data_paddr,
            end: PHYSTOP,
            perm: ent_flag::WRITABLE,
        },
        // more devices
        Kmap {
            virt: DEVSPACE,
            start: PAddr::from_raw(DEVSPACE.raw()),
            end: PAddr::from_raw(0),
            perm: ent_flag::WRITABLE,
        },
    ];

    let pg_dir = kalloc::kalloc()? as *mut PageDirectory;
    log!("setup_kvm: pg_dir = {:p}", pg_dir);

    unsafe { rlibc::memset(pg_dir as *mut u8, 0, PAGE_SIZE) };
    if p2v(PHYSTOP) > DEVSPACE {
        panic!("PHYSTOP too high");
    }
    let pg_dir = unsafe { &mut *pg_dir };
    for k in &kmap {
        if map_pages(
            pg_dir,
            k.virt.cast(),
            k.end.raw().wrapping_sub(k.start.raw()),
            k.start,
            k.perm,
        )
        .is_none()
        {
            println!(print_color::LIGHT_RED; "map_pages fail");
            free_vm(pg_dir);
            return None;
        }
    }
    Some(pg_dir)
}

use super::spinlock::SpinMutex;
static KPG_DIR: SpinMutex<*mut PageDirectory> = SpinMutex::new("kpgdir", core::ptr::null_mut());
/// Allocate one page table for the machine for the kernel address
/// space for scheduler processes.
pub fn kvmalloc() {
    let kpg_dir = setup_kvm().expect("kvmalloc failed");
    {
        let mut guard = KPG_DIR.lock();
        *guard = kpg_dir;
    }
}

/// Deallocate user pages to bring the process size from old_sz to
/// new_sz.  old_sz and new_sz need not be page-aligned, nor does new_sz
/// need to be less than old_sz.  old_sz can be larger than the actual
/// process size.  Returns the new process size.
fn dealloc_uvm(_pg_dir: &mut PageDirectory, _old_sz: usize, _new_sz: usize) {
    todo!()
}

/// Free a page table and all the physical memory pages in the user part.
fn free_vm(pg_dir: *mut PageDirectory) {
    if pg_dir.is_null() {
        panic!("free_vm: no pg_dir");
    }
    let pg_dir = unsafe { &mut *pg_dir };
    dealloc_uvm(pg_dir, KERNBASE.raw(), 0);
    for ent in pg_dir.iter() {
        if ent.flags_check(ent_flag::PRESENT) {
            let v = p2v(ent.addr());
            let v: VAddr<Page> = v.cast();
            kalloc::kfree(v.mut_ptr());
        }
    }
    kalloc::kfree(pg_dir as *mut PageDirectory as *mut Page);
}