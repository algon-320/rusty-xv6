#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(global_asm)]
#![feature(asm)]
#![feature(start)]
#![allow(clippy::identity_op)]

extern crate rlibc;

mod memory;

use memory::pg_dir::{ent_flag, PageDirEntry, NPDENTRIES};
use utils::assigned_array;
use utils::x86;

#[used] // must not be removed
#[no_mangle]
#[link_section = ".rodata.entry_page_dir"]
pub static entry_page_dir: [PageDirEntry; NPDENTRIES] = assigned_array![
    PageDirEntry::zero(); NPDENTRIES;

    // Map VA's [0, 4MB) to PA's [0, 4MB)
    [0] =
        PageDirEntry::large_page(0x00000000,
                ent_flag::WRITABLE | ent_flag::PRESENT),

    // Map VA's [KERNBASE, KERNBASE + 4MB) to PA's [0, 4MB)
    [0x80000000 >> 22] =
        PageDirEntry::large_page(0x00000000,
                ent_flag::WRITABLE | ent_flag::PRESENT)
];

fn print(s: &str) {
    const VGA_BUF: *mut u8 = 0xb8000 as _;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        unsafe {
            *VGA_BUF.add(i * 2 + 0) = b;
            *VGA_BUF.add(i * 2 + 1) = 0b00001010; // green
        }
    }
}
fn print_u64(mut x: u64) {
    let mut buf = [b'.'; 20];
    let mut i = 0;
    while x > 0 {
        let dig = x % 10;
        buf[i] = core::char::from_digit(dig as _, 10).unwrap() as u8;
        i += 1;
        x /= 10;
    }
    buf[..i].reverse();
    print(unsafe { core::str::from_utf8_unchecked(&buf[..i]) });
}

#[no_mangle]
pub extern "C" fn main() {
    for i in 0..10000000000 {
        print_u64(i);
    }
    todo!()
}

#[panic_handler]
#[no_mangle]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    x86::nop();
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
