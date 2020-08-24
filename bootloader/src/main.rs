#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(global_asm)]

use utils::x86;
mod elf;

const SECTOR_SIZE: usize = 512;
type Sector = [u8; SECTOR_SIZE];

/// Read a single sector at offset into dst.
fn read_sector(dst: *mut Sector, offset: usize) {
    fn wait_disk() {
        // Wait for disk ready
        while (x86::inb(0x01F7) & 0xC0) != 0x40 {
            x86::nop();
        }
    }

    wait_disk();
    x86::outb(0x01F2, 0x01); // count = 1
    x86::outb(0x01F3, (offset & 0xFF) as u8);
    x86::outb(0x01F4, ((offset >> 8) & 0xFF) as u8);
    x86::outb(0x01F5, ((offset >> 16) & 0xFF) as u8);
    x86::outb(0x01F6, (((offset >> 24) | 0xE0) & 0xFF) as u8);
    x86::outb(0x01F7, 0x20); // cmd 0x20 - read sectors

    // Read the data.
    wait_disk();
    x86::insl(0x01F0, dst as *mut u32, SECTOR_SIZE / 4);
}

/// Read 'count' bytes at 'offset' from kernel into physical address 'pa'.
/// Might copy more than asked.
unsafe fn read_segment(pa: *mut u8, count: usize, offset: usize) {
    let end_pa = pa.add(count);
    let pa = pa.sub(offset % SECTOR_SIZE);
    let mut pa = pa as *mut Sector;

    let mut offset = (offset / SECTOR_SIZE) + 1;
    while pa.cast() < end_pa {
        read_sector(pa, offset);
        pa = pa.add(1);
        offset += 1;
    }
}

#[no_mangle]
fn boot_main() {
    let elf = 0x00010000 as *mut elf::ElfHeader;

    unsafe {
        // Read first page of disk
        read_segment(elf as *mut u8, 4096, 0);

        // Is this an ELF executable?
        if !(*elf).verify() {
            return;
        }

        // Load each program segment (ignores ph flags).
        for ph in (*elf).prog_headers() {
            let pa = ph.p_paddr;
            read_segment(pa, ph.p_filesz, ph.p_offset);
            if ph.p_memsz > ph.p_filesz {
                // fill with zero
                x86::stosb(pa.add(ph.p_filesz), 0, ph.p_memsz - ph.p_filesz);
            }
        }

        // Go to kernel
        ((*elf).e_entry)();
    }
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
.code16
.global start

start:
    # Disable interrupts
    cli

    # Make data segment registers 0
    xorw    %ax, %ax
    movw    %ax, %ds
    movw    %ax, %es
    movw    %ax, %ss

    # Enable A20 bus line
    call    enable_A20

    # Switch to protected mode
    lgdt    gdtdesc
    # set protect mode bit (first bit of cr0)
    movl    %cr0, %eax
    orl     $0x00000001, %eax
    movl    %eax, %cr0

    ljmp    $(1 << 3), $start32

# See: https://wiki.osdev.org/A20_Line
enable_A20:
    call    A20_wait1
    movb    $0xAD, %al
    outb    %al, $0x64

    call    A20_wait1
    movb    $0xD0, %al
    outb    %al, $0x64

    call    A20_wait2
    inb     $0x60, %al
    push    %eax

    call    A20_wait1
    movb    $0xD1, %al
    outb    %al, $0x64

    call    A20_wait1
    pop     %eax
    orb     $0x02, %al
    outb    %al, $0x60

    call    A20_wait1
    movb    $0xAE, %al
    outb    %al, $0x64
    
    call    A20_wait1
    ret

A20_wait1:
    inb     $0x64, %al
    testb   $0x02, %al
    jnz     A20_wait1
    ret

A20_wait2:
    inb     $0x64, %al
    testb   $0x01, %al
    jz      A20_wait2
    ret

.code32
start32:
    # Set up segment registers
    movw    $(2 << 3), %ax
    movw    %ax, %ds
    movw    %ax, %es
    movw    %ax, %ss
    movw    $00, %ax
    movw    %ax, %fs
    movw    %ax, %gs

    # Set up the stack pointer
    movl    $start, %esp

    # Now we can go to the rust function
    call    boot_main

    # boot_main never return
spin:
    jmp     spin

# See: https://wiki.osdev.org/Global_Descriptor_Table
.p2align  2
gdt:
    # null descriptor
    .word   0x0000, 0x0000
    .byte   0x00, 0x00, 0x00, 0x00

    # kernel code
    .word   0xFFFF, 0x0000
    .byte   0x00, 0x9A, 0xCF, 0x00

    # kernel data+stack
    .word   0xFFFF, 0x0000
    .byte   0x00, 0x92, 0xCF, 0x00
gdtdesc:
    .word   (gdtdesc - gdt - 1)  # sizeof(gdt) - 1
    .long   gdt                  # address of gdt
"#}
