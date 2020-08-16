#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(global_asm)]
#![feature(llvm_asm)]
#![feature(start)]
#![feature(custom_test_frameworks)]
#![feature(const_raw_ptr_to_usize_cast)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::identity_op)]

extern crate rlibc;

mod kalloc;
mod lapic;
mod memory;
mod mp;
mod proc;
mod spinlock;
mod vm;

use utils::prelude::*;
use utils::{assigned_array, x86};

use memory::p2v;
use memory::pg_dir::{ent_flag, PageDirEntry, NPDENTRIES};

#[used] // must not be removed
#[no_mangle]
#[link_section = ".rodata.entry_page_dir"]
pub static entry_page_dir: [PageDirEntry; NPDENTRIES] = assigned_array![
    PageDirEntry::zero(); NPDENTRIES;

    // Map VA's [0, 4MB) to PA's [0, 4MB)
    [0] =
        PageDirEntry::new_large_page(
                unsafe { PAddr::from_raw_unchecked(0x00000000) },
                ent_flag::WRITABLE | ent_flag::PRESENT),

    // Map VA's [KERNBASE, KERNBASE + 4MB) to PA's [0, 4MB)
    [memory::KERNBASE.raw() >> 22] =
        PageDirEntry::new_large_page(
                unsafe { PAddr::from_raw_unchecked(0x00000000) },
                ent_flag::WRITABLE | ent_flag::PRESENT)
];

extern "C" {
    ///  first address after kernel loaded from ELF file
    static kernel_end: core::ffi::c_void;
}

#[no_mangle]
pub extern "C" fn main() {
    #[cfg(test)]
    {
        test_main();
    }
    #[cfg(not(test))]
    {
        {
            let kernel_end_addr = VAddr::from_raw(unsafe { &kernel_end } as *const _ as usize);
            let heap_end = p2v(PAddr::from_raw(4 * 1024 * 1024));
            kalloc::init1(kernel_end_addr, heap_end);
        };
        vm::kvmalloc();
        mp::init();
        lapic::init();
    }
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test_case]
    fn address_round_up() {
        let addr: PAddr<u8> = PAddr::from_raw(20).round_up(4096);
        let expected = PAddr::from_raw(4096);
        assert_eq!(addr, expected);

        let addr: PAddr<u8> = PAddr::from_raw(4096).round_up(4096);
        let expected = PAddr::from_raw(4096);
        assert_eq!(addr, expected);

        let addr: PAddr<u8> = PAddr::from_raw(0usize.wrapping_sub(1)).round_up(4096);
        let expected = PAddr::from_raw(0);
        assert_eq!(addr, expected);
    }
    #[test_case]
    fn address_round_down() {
        let addr: PAddr<u8> = PAddr::from_raw(20).round_down(4096);
        let expected = PAddr::from_raw(0);
        assert_eq!(addr, expected);

        let addr: PAddr<u8> = PAddr::from_raw(4100).round_down(4096);
        let expected = PAddr::from_raw(4096);
        assert_eq!(addr, expected);

        let addr: PAddr<u8> = PAddr::from_raw(0).round_down(4096);
        let expected = PAddr::from_raw(0);
        assert_eq!(addr, expected);
    }

    #[test_case]
    fn x86_xchg() {
        let mut x = 123u32;
        let y = x86::xchgl(&mut x, 456u32);
        assert_eq!(y, 123);
        assert_eq!(x, 456);
    }
}

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!(print_color::LIGHT_GREEN; "all tests passed!");
}

#[panic_handler]
#[no_mangle]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!(print_color::LIGHT_RED; "{}", info);
    loop {}
}

#[lang = "eh_personality"]
#[no_mangle]
fn eh_personality() -> ! {
    x86::nop();
    loop {}
}

global_asm! {r#"
.set KERNBASE,      0x80000000  # First kernel virtual address
.set CR0_WP,        0x00010000  # Write Protect
.set CR0_PG,        0x80000000  # Paging
.set CR4_PSE,       0x00000010  # Page size extension

.set KSTACKSIZE,    4096 * 2    # Size of per-process kernel stack

.p2align 2
.text

# TODO support multiboot

.globl _start
_start = (entry - KERNBASE)

.globl entry
entry:
    # Turn on page size extension for 4MB pages
    movl    %cr4, %eax
    orl     $(CR4_PSE), %eax
    movl    %eax, %cr4

    # Set page directory
    movl    $(entry_page_dir - KERNBASE), %eax
    movl    %eax, %cr3

    # Turn on paging
    movl    %cr0, %eax
    orl     $(CR0_PG|CR0_WP), %eax
    movl    %eax, %cr0

    # Set up the stack pointer
    movl    $(stack + KSTACKSIZE), %esp

    # Jump to main()
    mov     $main, %eax
    jmp     *%eax

.comm stack, KSTACKSIZE
"#}
