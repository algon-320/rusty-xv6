use super::memory::seg;
use core::cell::{RefCell, RefMut};
use core::sync::atomic::AtomicBool;
use utils::x86;

#[derive(Debug)]
pub struct Cpu {
    pub gdt: [seg::SegDesc; seg::NSEGS],
    pub num_cli: i32,
    pub int_enabled: bool,
    pub current_proc: *mut Process,
}
pub struct CpuShared {
    /// Local APIC ID
    pub apic_id: u8,
    pub started: AtomicBool,
    pub private: RefCell<Cpu>,
}
impl CpuShared {
    pub const fn zero() -> Self {
        Self {
            apic_id: 0,
            started: AtomicBool::new(false),
            private: RefCell::new(Cpu {
                gdt: seg::GDT_ZERO,
                num_cli: 0,
                int_enabled: false,
                current_proc: core::ptr::null_mut(),
            }),
        }
    }
}

/// maximum number of CPUs
const MAX_NCPU: usize = 8;
static mut _NCPU: usize = 0;
/// Should not access this directly. Use cpus() instead.
pub static mut _CPUS: [CpuShared; MAX_NCPU] = [
    CpuShared::zero(),
    CpuShared::zero(),
    CpuShared::zero(),
    CpuShared::zero(),
    CpuShared::zero(),
    CpuShared::zero(),
    CpuShared::zero(),
    CpuShared::zero(),
];
pub unsafe fn init_new_cpu() -> Option<&'static mut CpuShared> {
    if _NCPU == MAX_NCPU {
        None
    } else {
        _NCPU += 1;
        Some(&mut _CPUS[_NCPU - 1])
    }
}
pub fn cpus() -> &'static [CpuShared] {
    unsafe { &_CPUS[.._NCPU] }
}

/// Must be called with interrupts disabled
pub fn my_cpu_id() -> u8 {
    assert!(
        x86::read_eflags() & x86::eflags::FL_IF == 0,
        "my_cpu called with interrupts enabled"
    );

    let apic_id = super::lapic::lapic_id().expect("LAPIC is None");
    // APIC IDs are not guaranteed to be contiguous.
    cpus()
        .iter()
        .position(|cpu| cpu.apic_id == apic_id)
        .unwrap() as u8
}

pub fn my_cpu() -> RefMut<'static, Cpu> {
    assert!(
        x86::read_eflags() & x86::eflags::FL_IF == 0,
        "my_cpu called with interrupts enabled"
    );

    let apic_id = super::lapic::lapic_id().expect("LAPIC is None");
    // APIC IDs are not guaranteed to be contiguous.
    cpus()
        .iter()
        .find_map(|cpu| {
            if cpu.apic_id != apic_id {
                None
            } else {
                // log!("cpu {} is now borrowed", apic_id);
                Some(cpu.private.borrow_mut())
            }
        })
        .unwrap()
}

const MAX_NPROC: usize = 64;

#[derive(Debug, Copy, Clone)]
pub struct Process {}
impl Process {
    pub const fn zero() -> Self {
        Self {}
    }
}
use super::lock::spin::SpinMutex;
static PROC_TABLE: SpinMutex<[Process; MAX_NPROC]> =
    SpinMutex::new("ptable", [Process::zero(); MAX_NPROC]);

pub fn init() {
    //
}

pub fn user_init() {
    todo!()
}
