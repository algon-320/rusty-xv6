use super::memory::seg;
use utils::x86;

#[derive(Debug, Copy, Clone)]
pub struct Cpu {
    /// Local APIC ID
    pub apic_id: u8,
    pub gdt: [seg::SegDesc; seg::NSEGS],
    pub num_cli: i32,
    pub int_enabled: bool,
}
impl Cpu {
    pub const fn zero() -> Self {
        Self {
            apic_id: 0,
            gdt: seg::GDT_ZERO,
            num_cli: 0,
            int_enabled: false,
        }
    }
}

/// maximum number of CPUs
pub const MAX_NCPU: usize = 8;
pub(crate) static mut NCPU: usize = 0;
pub(crate) static mut CPUS: [Cpu; MAX_NCPU] = [
    Cpu::zero(),
    Cpu::zero(),
    Cpu::zero(),
    Cpu::zero(),
    Cpu::zero(),
    Cpu::zero(),
    Cpu::zero(),
    Cpu::zero(),
];

// Must be called with interrupts disabled
pub fn my_cpu_id() -> u8 {
    let ptr = my_cpu() as *mut _ as *const Cpu;
    let off = unsafe { CPUS.as_ptr() };
    unsafe { ptr.offset_from(off) as u8 }
}

pub fn my_cpu() -> &'static mut Cpu {
    assert!(
        x86::read_eflags() & x86::eflags::FL_IF == 0,
        "my_cpu called with interrupts enabled"
    );

    let apic_id = super::lapic::lapic_id().expect("LAPIC is None");
    // APIC IDs are not guaranteed to be contiguous. Maybe we should have
    // a reverse map, or reserve a register to store &CPUS[i].
    unsafe {
        for cpu in CPUS.iter_mut() {
            if cpu.apic_id == apic_id {
                return cpu;
            }
        }
    }
    panic!("unknown apic_id");
}
