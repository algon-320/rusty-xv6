use super::trap;
use core::ptr::{read_volatile, write_volatile};

pub(crate) static mut IOAPIC_ID: u8 = 0b01010101;
const IOAPIC: *mut IoApic = 0xFEC00000 as *mut IoApic;

/// IO APIC MMIO structure: write reg, then read or write data.
///
/// [reg, pad1, pad2, pad3, data] : [u32; 5]
type IoApic = [u32; 5];

const REG_OFFSET: usize = 0;
const DAT_OFFSET: usize = 4;

const REG_ID: u32 = 0x00;
const REG_VER: u32 = 0x01;
const REG_TABLE: u32 = 0x10; // 32-bit register for each IRQ

// The redirection table starts at REG_TABLE and uses
// two registers to configure each interrupt.
// The first (low) register in a pair contains configuration bits.
// The second (high) register contains a bitmask telling which

// CPUs can serve that interrupt.
/// Interrupt disabled
const INT_DISABLED: u32 = 0x00010000;
/// Level-triggered (vs edge-)
const INT_LEVEL: u32 = 0x00008000;
/// Active low (vs high)
const INT_ACTIVE_LOW: u32 = 0x00002000;
/// Destination is CPU id (vs APIC ID)
const INT_LOGICAL: u32 = 0x00000800;

fn read(reg: u32) -> u32 {
    unsafe {
        let reg_ptr = (IOAPIC as *mut u32).add(REG_OFFSET);
        let dat_ptr = (IOAPIC as *mut u32).add(DAT_OFFSET);
        write_volatile(reg_ptr, reg);
        read_volatile(dat_ptr)
    }
}
fn write(reg: u32, data: u32) {
    unsafe {
        let reg_ptr = (IOAPIC as *mut u32).add(REG_OFFSET);
        let dat_ptr = (IOAPIC as *mut u32).add(DAT_OFFSET);
        write_volatile(reg_ptr, reg);
        write_volatile(dat_ptr, data);
    }
}

pub fn init() {
    let max_intr = ((read(REG_VER) >> 16) & 0xFF) + 1;
    let id = (read(REG_ID) >> 24) as u8;
    if id != unsafe { IOAPIC_ID } {
        log!("ioapic::init id isn't equal to IOAPIC_ID; not a MP");
    }

    // Mark all interrupts edge-triggered, active high, disabled,
    // and not routed to any CPUs.
    for i in 0..max_intr {
        write(REG_TABLE + 2 * i + 0, INT_DISABLED | (trap::T_IRQ0 + i));
        write(REG_TABLE + 2 * i + 1, 0);
    }
}

pub fn enable(irq: u32, cpu_num: usize) {
    // Mark interrupt edge-triggered, active high,
    // enabled, and routed to the given cpu_num,
    // which happens to be that cpu's APIC ID.
    write(REG_TABLE + 2 * irq + 0, trap::T_IRQ0 + irq);
    write(REG_TABLE + 2 * irq + 1, (cpu_num as u32) << 24);
}
