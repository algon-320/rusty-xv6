#![allow(dead_code)]

/// Local APIC registers, divided by 4 for use as uint[] indices.
///
/// for more details:
///     Intel Software Developer Manual - 10.4 Table 10-1 Local APIC Register Address Map
#[repr(usize)]
enum LapicReg {
    /// ID (R/W)
    ID = 0x0020 / 4,
    /// Version (R)
    VER = 0x0030 / 4,
    /// Task Priority (R/W)
    TPR = 0x0080 / 4,
    /// EOI (W)
    EOI = 0x00B0 / 4,
    /// Spurious Interrupt Vector (R/W)
    SVR = 0x00F0 / 4,
    /// Error Status (R)
    ESR = 0x0280 / 4,
    /// Interrupt Command (R/W)
    ICRLO = 0x0300 / 4,
    /// Interrupt Command [63:32] (R/W)
    ICRHI = 0x0310 / 4,
    /// Local Vector Table 0 (TIMER) (R/W)
    TIMER = 0x0320 / 4,
    /// Performance Monitoring Counter LVT (R/W)
    PCINT = 0x0340 / 4,
    /// Local Vector Table 1 (LINT0) (R/W)
    LINT0 = 0x0350 / 4,
    /// Local Vector Table 2 (LINT1) (R/W)
    LINT1 = 0x0360 / 4,
    /// Local Vector Table 3 (ERROR) (R/W)
    ERROR = 0x0370 / 4,
    /// Timer Initial Count (R/W)
    TICR = 0x0380 / 4,
    /// Timer Current Count (R)
    TCCR = 0x0390 / 4,
    /// Timer Divide Configuration (R/W)
    TDCR = 0x03E0 / 4,
}
impl LapicReg {
    pub unsafe fn write(self, value: u32) {
        core::ptr::write_volatile(LAPIC.unwrap().add(self as usize), value);
        // wait for write to finish, by reading
        LapicReg::ID.read();
    }
    pub unsafe fn read(self) -> u32 {
        core::ptr::read_volatile(LAPIC.unwrap().add(self as usize))
    }
}

/// Unit Enable
const ENABLE: u32 = 0x00000100;
/// INIT/RESET
const INIT: u32 = 0x00000500;
/// Startup IPI
const STARTUP: u32 = 0x00000600;
/// Delivery status
const DELIVS: u32 = 0x00001000;
/// Assert interrupt (vs deassert)
const ASSERT: u32 = 0x00004000;
const DEASSERT: u32 = 0x00000000;
/// Level triggered
const LEVEL: u32 = 0x00008000;
/// Send to all APICs, including self.
const BCAST: u32 = 0x00080000;
const BUSY: u32 = 0x00001000;
const FIXED: u32 = 0x00000000;
/// divide counts by 1
const X1: u32 = 0x0000000B;
/// Periodic
const PERIODIC: u32 = 0x00020000;
/// Interrupt masked
const MASKED: u32 = 0x00010000;

// TODO: move them to trap.rs
mod trap {
    // These are arbitrarily chosen, but with care not to overlap
    // processor defined exceptions or interrupt vectors.
    pub const T_SYSCALL: u32 = 64; // system call
    pub const T_DEFAULT: u32 = 500; // catchall
    pub const T_IRQ0: u32 = 32; // IRQ 0 corresponds to int T_IRQ
    pub const IRQ_TIMER: u32 = 0;
    pub const IRQ_KBD: u32 = 1;
    pub const IRQ_COM1: u32 = 4;
    pub const IRQ_IDE: u32 = 14;
    pub const IRQ_ERROR: u32 = 19;
    pub const IRQ_SPURIOUS: u32 = 31;
}

pub(crate) static mut LAPIC: Option<*mut u32> = None;
// Initialized in mp::init()

pub fn init() {
    if unsafe { LAPIC.is_none() } {
        return;
    }

    unsafe {
        // Enable local APIC; spurious interrupt vector.
        LapicReg::SVR.write(ENABLE | (trap::T_IRQ0 + trap::IRQ_SPURIOUS));

        // The timer repeatedly counts down at bus frequency
        // from lapic[TICR] and then issues an interrupt.
        // If xv6 cared more about precise timekeeping,
        // TICR would be calibrated using an external time source.
        LapicReg::TDCR.write(X1);
        LapicReg::TIMER.write(PERIODIC | (trap::T_IRQ0 + trap::IRQ_TIMER));
        LapicReg::TICR.write(10000000);

        // Disable logical interrupt lines.
        LapicReg::LINT0.write(MASKED);
        LapicReg::LINT1.write(MASKED);

        // Disable performance counter overflow interrupts
        // on machines that provide that interrupt entry.
        if ((LapicReg::VER.read() >> 16) & 0xFF) >= 4 {
            LapicReg::PCINT.write(MASKED);
        }

        // Map error interrupt to IRQ_ERROR
        LapicReg::ERROR.write(trap::T_IRQ0 + trap::IRQ_ERROR);

        // Clear error status register (requires back-to-back writes).
        LapicReg::ESR.write(0);
        LapicReg::ESR.write(0);

        // Ack any outstanding interrupts.
        LapicReg::EOI.write(0);

        // Send an Init Level De-Assert to synchronize arbitration ID's.
        LapicReg::ICRHI.write(0);
        LapicReg::ICRLO.write(BCAST | INIT | LEVEL);
        while LapicReg::ICRLO.read() & DELIVS > 0 {}

        // Enable interrupts on the APIC (but not on the processor).
        LapicReg::TPR.write(0);
    }
}
