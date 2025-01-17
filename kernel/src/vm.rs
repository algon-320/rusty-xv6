use super::memory::pg_dir::{
    self, ent_flag, PageDirEntry, PageDirectory, PageTable, PageTableEntry,
};
use super::memory::{p2v, v2p, Page};
use super::memory::{DEVSPACE, EXTMEM, KERNBASE, KERNLINK, PAGE_SIZE, PHYSTOP};
use alloc::boxed::Box;
use lazy_static::lazy_static;
use utils::prelude::*;
use utils::x86;

struct Kmap {
    virt: VAddr<Page>,
    start: PAddr<Page>,
    end: PAddr<Page>,
    perm: u32,
}

/// Set up CPU's kernel segment descriptors.
/// Run once on entry on each CPU.
pub fn seginit() {
    use super::memory::seg::*;

    // Map "logical" addresses to virtual addresses using identity map.
    // Cannot share a CODE descriptor for both kernel and user
    // because it would have to have dpl::USER,
    // but the CPU forbids an interrupt from CPL=0 to DPL=3.
    let mut c = super::proc::my_cpu();
    let type_code = seg_type::STA_X | seg_type::STA_R;
    let type_data = seg_type::STA_W;
    c.gdt[SEG_KCODE] = SegDesc::seg(type_code, 0, 0xFFFFFFFF, 0);
    c.gdt[SEG_KDATA] = SegDesc::seg(type_data, 0, 0xFFFFFFFF, 0);
    c.gdt[SEG_UCODE] = SegDesc::seg(type_code, 0, 0xFFFFFFFF, dpl::USER);
    c.gdt[SEG_UDATA] = SegDesc::seg(type_data, 0, 0xFFFFFFFF, dpl::USER);
    x86::lgdt(
        c.gdt.as_ptr() as *const u8,
        core::mem::size_of::<[SegDesc; NSEGS]>() as u16,
    );
}

// Return the reference of the PTE in page table pg_dir
// that corresponds to virtual address va.  If alloc!=0,
// create any required page table pages.
fn walk_page_dir(
    pg_dir: &mut PageDirectory,
    va: VAddr<Page>,
    alloc: bool,
) -> Option<&mut PageTableEntry> {
    use pg_dir::{pdx, ptx};
    let pde = &mut pg_dir[pdx(va)];
    let pg_tab = if pde.flags_check(ent_flag::PRESENT) {
        p2v(pde.addr())
    } else {
        if !alloc {
            return None;
        }
        let pg_tab = PageTable::zero_boxed();
        // this leak will be retrieved in 'free_vm' and deallocated.
        let pg_tab = VAddr::from(Box::into_raw(pg_tab));

        // The permissions here are overly generous, but they can
        // be further restricted by the permissions in
        // the page table entries, if necessary.
        *pde = PageDirEntry::new_table(
            v2p(pg_tab).raw() as u32,
            ent_flag::PRESENT | ent_flag::WRITABLE | ent_flag::USER,
        );
        // assert!(*pde & mmu::PteFlags::PRESENT.bits() != 0);
        pg_tab
    };
    let pg_tab = unsafe { &mut *pg_tab.mut_ptr() };
    Some(&mut pg_tab[ptx(va)])
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
    }
    Some(())
}

/// Set up kernel part of a page table.
pub fn setup_kvm() -> Option<Box<PageDirectory>> {
    let data_vaddr = {
        extern "C" {
            static data: u8;
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

    let mut pg_dir = PageDirectory::zero_boxed();
    if p2v(PHYSTOP) > DEVSPACE {
        panic!("PHYSTOP too high");
    }
    {
        for k in &kmap {
            if map_pages(
                &mut pg_dir,
                k.virt.cast(),
                k.end.raw().wrapping_sub(k.start.raw()),
                k.start,
                k.perm,
            )
            .is_none()
            {
                println!(super::console::print_color::LIGHT_RED; "map_pages fail");
                free_vm(pg_dir);
                return None;
            }
        }
    }
    Some(pg_dir)
}

lazy_static! {
    static ref KPG_DIR: &'static PageDirectory = Box::leak(setup_kvm().expect("kvmalloc failed"));
}

/// Allocate one page table for the machine for the kernel address
/// space for scheduler processes.
pub fn kvmalloc() {
    lazy_static::initialize(&KPG_DIR);

    // Now, we switch the page table from entry_page_dir to kpg_dir
    switch_kvm();
}

pub fn switch_kvm() {
    x86::lcr3(v2p(VAddr::from(*KPG_DIR)).raw() as u32);
}

/// Free a page table and all the physical memory pages in the user part.
fn free_vm(mut pg_dir: Box<PageDirectory>) {
    uvm::dealloc(&mut pg_dir, KERNBASE.raw(), 0);
    for ent in pg_dir
        .iter()
        .filter(|ent| ent.flags_check(ent_flag::PRESENT))
    {
        let v = p2v(ent.addr()).cast::<Page>();
        let t = unsafe { Box::from_raw(v.mut_ptr()) };
        drop(t);
    }
}

pub mod uvm {
    use super::*;
    use crate::lock::cli;
    use crate::memory::{seg, v2p, KSTACKSIZE};
    use crate::proc::{my_cpu, ProcessRef, TaskState};
    use core::mem::size_of;
    use utils::x86;

    /// Load the init_code into address 0 of pg_dir.
    /// the size of init_code must be less than a page.
    pub fn init(pg_dir: &mut pg_dir::PageDirectory, init_code: &[u8]) {
        assert!(init_code.len() < PAGE_SIZE);
        let mem = crate::kalloc::kalloc().unwrap().as_ptr() as *mut u8;
        unsafe { rlibc::memset(mem, 0, PAGE_SIZE) };
        map_pages(
            pg_dir,
            VAddr::from_raw(0),
            PAGE_SIZE,
            v2p(VAddr::from(mem as *mut Page)),
            ent_flag::WRITABLE | ent_flag::USER,
        );
        unsafe { core::ptr::copy_nonoverlapping(init_code.as_ptr(), mem, init_code.len()) };
    }

    /// Switch TSS and h/w page table to correspond to process p.
    pub fn switch(p: &ProcessRef) {
        let p = p.lock();
        assert!(p.is_valid(), "switch_uvm: no process");

        cli(|| unsafe {
            let mut cpu = my_cpu();
            cpu.gdt[seg::SEG_TSS] = seg::SegDesc::tss(
                seg::seg_type::STS_T32A,
                &cpu.task_state as *const _ as u32,
                (size_of::<TaskState>() - 1) as u32,
                0,
            );
            cpu.task_state.ss0 = (seg::SEG_KDATA as u16) << 3;
            cpu.task_state.esp0 = p.kernel_stack.add(KSTACKSIZE) as u32;
            // setting IOPL=0 in eflags *and* iomb beyond the tss segment limit
            // forbids I/O instructions (e.g., inb and outb) from user space
            cpu.task_state.iomb = 0xFFFF;
            x86::ltr((seg::SEG_TSS as u16) << 3);
            x86::lcr3(v2p(VAddr::from(p.pg_dir.as_ref())).raw() as u32);
        });
    }

    /// Deallocate user pages to bring the process size from old_sz to
    /// new_sz.  old_sz and new_sz need not be page-aligned, nor does new_sz
    /// need to be less than old_sz.  old_sz can be larger than the actual
    /// process size.  Returns the new process size.
    pub fn dealloc(_pg_dir: &mut PageDirectory, _old_sz: usize, _new_sz: usize) {
        todo!()
    }
}
