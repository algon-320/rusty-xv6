use super::memory::p2v;
use utils::prelude::*;
use utils::x86;

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

pub(crate) static mut LAPIC: Option<*mut u32> = None;
// Initialized in mp::init()

pub fn init() {
    if unsafe { LAPIC.is_none() } {
        return;
    }

    unsafe {
        use super::trap;

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

pub fn lapic_id() -> Option<u8> {
    unsafe { LAPIC? };
    Some(((unsafe { LapicReg::ID.read() } >> 24) & 0xFF) as u8)
}

/// Spin for a given number of microseconds.
/// On real hardware would want to tune this dynamically.
pub fn micro_delay(_us: u32) {}

const CMOS_PORT: u16 = 0x70;
const CMOS_RETURN: u16 = 0x71;

// Start additional processor running entry code at addr.
// See Appendix B of MultiProcessor Specification.
pub fn start_ap(apic_id: u8, addr: PAddr<*mut core::ffi::c_void>) {
    dbg!(apic_id, addr.ptr());

    // "The BSP must initialize CMOS shutdown code to 0AH
    // and the warm reset vector (DWORD based at 40:67) to point at
    // the AP startup code prior to the [universal startup algorithm]."
    x86::outb(CMOS_PORT + 0, 0x0F); // offset 0xF is shutdown code
    x86::outb(CMOS_PORT + 1, 0x0A);
    let wrv = {
        let p = unsafe { PAddr::<u16>::from_raw_unchecked(0x40 << 4 | 0x67) };
        p2v(p).mut_ptr()
    };
    unsafe {
        *wrv.add(0) = 0;
        *wrv.add(1) = ((addr.raw() >> 4) & 0xFFFF) as u16;
    }

    // "Universal startup algorithm."
    // Send INIT (level-triggered) interrupt to reset other CPU.
    unsafe {
        LapicReg::ICRHI.write((apic_id as u32) << 24);
        LapicReg::ICRLO.write(INIT | LEVEL | ASSERT);
        micro_delay(200);
        LapicReg::ICRLO.write(INIT | LEVEL);
        micro_delay(100);
    }

    // Send startup IPI (twice!) to enter code.
    // Regular hardware is supposed to only accept a STARTUP
    // when it is in the halted state due to an INIT.  So the second
    // should be ignored, but it is part of the official Intel algorithm.
    // Bochs complains about the second one.  Too bad for Bochs.
    for _ in 0..2 {
        unsafe {
            LapicReg::ICRHI.write((apic_id as u32) << 24);
            LapicReg::ICRLO.write(STARTUP | (addr.raw() as u32 >> 12));
            micro_delay(200);
        }
    }
}
