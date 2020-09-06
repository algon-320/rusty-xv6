#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(global_asm)]
#![feature(llvm_asm)]
#![feature(start)]
#![feature(custom_test_frameworks)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(ptr_offset_from)]
#![feature(const_in_array_repeat_expressions)]
#![feature(ptr_internals)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::identity_op)]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate rlibc;

#[macro_use]
mod console;
mod fs;
mod ide;
mod ioapic;
mod kalloc;
mod lapic;
mod lock;
mod memory;
mod mp;
mod pic_irq;
mod proc;
mod trap;
mod uart;
mod vm;

use utils::prelude::*;
use utils::{assigned_array, x86};

use memory::pg_dir::{ent_flag, PageDirEntry, PageDirectory, NPDENTRIES};
use memory::{p2v, v2p};

#[used] // must not be removed
#[no_mangle]
pub static entry_page_dir: PageDirectory = PageDirectory(assigned_array![
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
                ent_flag::WRITABLE | ent_flag::PRESENT),

    [0xFEC00000 >> 22] =
        PageDirEntry::new_large_page(
                unsafe { PAddr::from_raw_unchecked(0xFEC00000) },
                ent_flag::WRITABLE | ent_flag::PRESENT)
]);

extern "C" {
    ///  first address after kernel loaded from ELF file
    static kernel_end: u8;
}

#[cfg(test)]
#[no_mangle]
pub extern "C" fn main() -> ! {
    console::vga::clear_screen();
    ioapic::init();
    uart::init();
    test_main();
    x86::outb(0xF4, 0x0); // exit qemu
    loop {}
}
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn main() -> ! {
    console::vga::clear_screen();
    let pre_alloc_lim = PAddr::from_raw(4 * 1024 * 1024);
    kalloc::init1(
        VAddr::from_raw(unsafe { &kernel_end } as *const _ as usize),
        p2v(pre_alloc_lim),
    ); // phys page allocator
    vm::kvmalloc(); // kernel page table
    mp::init(); // detect other processors
    lapic::init(); // interrupt controller
    vm::seginit(); // segment descriptors
    pic_irq::init(); // disable pic
    ioapic::init(); // another interrupt controller
    console::init(); // console hardware
    uart::init(); // serial port
    uart::puts("xv6...\n"); // Announce that we're here.
    proc::init(); // process table
    trap::init(); // trap vectors
    fs::bcache::init(); // buffer cache
    ide::init(); // disk

    start_others(); // start other processors

    // must come after start_others()
    kalloc::init2(p2v(pre_alloc_lim), p2v(memory::PHYSTOP).cast());
    proc::user_init(); // first user process
    mp_main(); // finish this processor's setup
}

// Other CPUs jump here
#[no_mangle]
extern "C" fn mp_enter() {
    vm::switch_kvm();
    vm::seginit();
    lapic::init();
    mp_main();
}

// Common CPU setup code.
fn mp_main() -> ! {
    use core::sync::atomic::Ordering;
    use proc::{cpus, my_cpu_id};
    log!("cpu{}: starting", my_cpu_id());
    trap::idt_init(); // load idt register
    let id = my_cpu_id() as usize;
    cpus()[id].started.store(true, Ordering::SeqCst); // tell start_others() we're up
    proc::scheduler(); // start running processes
}

fn start_others() {
    use core::ffi::c_void;
    use memory::KSTACKSIZE;
    use proc::{cpus, my_cpu_id};
    debug_assert_eq!(core::mem::size_of::<*mut c_void>(), 4);

    let main_cpu_id = my_cpu_id() as usize;
    for (idx, cpu) in cpus().iter().enumerate() {
        if idx == main_cpu_id {
            continue;
        }

        let stack = kalloc::kalloc().unwrap() as *mut c_void;
        let code = p2v(PAddr::<*mut c_void>::from_raw(0x7000));
        unsafe {
            let code = code.mut_ptr();
            *code.sub(1) = stack.add(KSTACKSIZE);
            *code.sub(2) = core::mem::transmute(mp_enter as extern "C" fn());
            *code.sub(3) = v2p(VAddr::from(entry_page_dir.as_ptr())).cast().mut_ptr();
        }

        lapic::start_ap(cpu.apic_id, v2p(code));

        use core::sync::atomic::{spin_loop_hint, Ordering};
        while !cpu.started.load(Ordering::SeqCst) {
            spin_loop_hint();
        }
    }
}

#[panic_handler]
#[no_mangle]
fn panic(info: &core::panic::PanicInfo) -> ! {
    x86::cli(); // stop interruption
    println!(console::print_color::LIGHT_RED; "{}", info);

    // exit immediately if we are under test mode
    #[cfg(test)]
    x86::outb(0xF4, 0x1); // exit qemu with error

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

.set STACK_SIZE,    4096 * 2    # Additional space for logging

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
    movl    $(stack + STACK_SIZE), %esp

    # Jump to main()
    mov     $main, %eax
    jmp     *%eax

.comm stack, STACK_SIZE
"#}

global_asm! {r#"
.set CR0_PE,        0x00000001  # Protection Enable
.set CR0_WP,        0x00010000  # Write Protect
.set CR0_PG,        0x80000000  # Paging
.set CR4_PSE,       0x00000010  # Page size extension

.set SEG_KCODE,     1  # Kernel code
.set SEG_KDATA,     2  # Kernel data + stack

# Save the current destination section and switch to '.text.ap.start'
.pushsection .text.ap.start,"ax"

.code16
.globl ap_start
ap_start:
    cli

    xorw    %ax, %ax
    movw    %ax, %ds
    movw    %ax, %ss
    movw    %ax, %es

    # Switch to protected mode
    lgdt    ap_gdtdesc
    # set protect mode bit (first bit of cr0)
    movl    %cr0, %eax
    orl     $CR0_PE, %eax
    movl    %eax, %cr0

    ljmp    $(SEG_KCODE << 3), $(ap_start32)

.code32
ap_start32:
    # Set up the protected-mode data segment registers
    movw    $(SEG_KDATA << 3), %ax  # Our data segment selector
    movw    %ax, %ds                # -> DS: Data Segment
    movw    %ax, %es                # -> ES: Extra Segment
    movw    %ax, %ss                # -> SS: Stack Segment
    movw    $0, %ax                 # Zero segments not ready for use
    movw    %ax, %fs                # -> FS
    movw    %ax, %gs                # -> GS

    # Turn on page size extension for 4MiB pages
    movl    %cr4, %eax
    orl     $(CR4_PSE), %eax
    movl    %eax, %cr4

    # Use entrypgdir as our initial page table
    movl    (ap_start - 12), %eax
    movl    %eax, %cr3

    # Turn on paging
    movl    %cr0, %eax
    orl     $(CR0_PE | CR0_PG | CR0_WP), %eax
    movl    %eax, %cr0

    # Switch to the stack allocated by start_others()
    movl    (ap_start - 4), %esp
    # Call mp_enter()
    call    *(ap_start - 8)

    # never return to here
    movw    $0x8a00, %ax
    movw    %ax, %dx
    outw    %ax, %dx
    movw    $0x8ae0, %ax
    outw    %ax, %dx
ap_spin:
    jmp     ap_spin

# align multiple of 4
.p2align 2
ap_gdt:
    # null descriptor
    .word   0x0000, 0x0000
    .byte   0x00, 0x00, 0x00, 0x00

    # Executable, Readable, [0, 0xFFFFFFFF]
    .word   0xFFFF, 0x0000
    .byte   0x00, 0x9A, 0xCF, 0x00

    # Writable,  [0, 0xFFFFFFFF]
    .word   0xFFFF, 0x0000
    .byte   0x00, 0x92, 0xCF, 0x00

ap_gdtdesc:
  .word   (ap_gdtdesc - ap_gdt - 1)
  .long   ap_gdt

# Restore previous destination
.popsection
"#}

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!(console::print_color::LIGHT_GREEN; "all tests passed!");
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
