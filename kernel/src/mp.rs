/// Multiprocessor support
/// Search memory for MP description structures.
/// http://developer.intel.com/design/pentium/datashts/24201606.pdf
use super::memory::p2v;
use core::mem::size_of;
use utils::prelude::*;
use utils::x86;

/// MP Floating Pointer Structure
#[repr(C)]
struct Mp {
    /// b"_MP_"
    signature: [u8; 4],
    /// physical address of MP config table
    conf_addr: PAddr<MpConf>,
    /// 1
    length: u8,
    /// [14]
    spec_rev: u8,
    /// all bytes must add up to 0
    checksum: u8,
    /// MP system config type
    config_type: u8,
    imcr: u8,
    _reserved: [u8; 3],
}
impl Mp {
    /// Check whether the following conditions meet:
    /// 1. Is the signature "_MP_" ?
    /// 2. Is the check sum 0?
    pub fn verify(&self) -> bool {
        let p = self as *const _ as *const u8;
        self.signature == *b"_MP_" && unsafe { sum(p, size_of::<Mp>()) } == 0
    }
    pub fn get_conf(&self) -> Option<&MpConf> {
        if self.conf_addr.is_null() {
            return None;
        }
        let conf = unsafe { &*p2v(self.conf_addr).ptr() };
        if conf.verify() {
            Some(conf)
        } else {
            None
        }
    }
}

/// Configuration table header
#[repr(C)]
struct MpConf {
    /// "PCMP"
    signature: [u8; 4],
    /// total table length
    length: u16,
    /// [14]
    version: u8,
    /// all bytes must add up to 0
    checksum: u8,
    /// product id
    product: [u8; 20],
    /// OEM table pointer
    oem_table: *mut u32,
    /// OEM table length
    oem_length: u16,
    /// entry count
    entry: u16,
    /// address of local APIC
    lapic_addr: *mut u32,
    /// extended table length
    x_length: u16,
    /// extended table checksum
    x_checksum: u8,
    _reserved: u8,
}
impl MpConf {
    pub fn verify(&self) -> bool {
        let p = self as *const _ as *const u8;
        self.signature == *b"PCMP" && unsafe { sum(p, self.length as usize) } == 0
    }
}

mod proc_ent_type {
    /// One per processor
    pub const MPPROC: u8 = 0x00;
    /// One per bus
    pub const MPBUS: u8 = 0x01;
    /// One per I/O APIC
    pub const MPIOAPIC: u8 = 0x02;
    /// One per bus interrupt source
    pub const MPIOINTR: u8 = 0x03;
    /// One per system interrupt source
    pub const MPLINTR: u8 = 0x04;
}

/// Processor table entry
#[repr(C)]
struct MpProc {
    /// entry type (0)
    entry_type: u8,
    /// local APIC id
    apic_id: u8,
    /// local APIC version
    version: u8,
    /// CPU flags
    flags: u8,
    /// CPU signature
    signature: [u8; 4],
    /// feature flags from CPUID instruction
    feature: u32,
    _reserved: [u8; 8],
}

/// I/O APIC table entry
#[repr(C)]
struct MpIoApic {
    /// entry type (2)
    entry_type: u8,
    /// I/O APIC id
    apic_no: u8,
    /// I/O APIC version
    version: u8,
    /// I/O APIC flags
    flags: u8,
    /// I/O APIC address
    addr: *mut u32,
}

/// sum up given bytes
unsafe fn sum(p: *const u8, len: usize) -> u8 {
    core::slice::from_raw_parts(p, len)
        .iter()
        .fold(0, |acc, x| acc.wrapping_add(*x))
}

/// Search for the MP Floating Pointer Structure, which according to the
/// spec is in one of the following three locations:
/// 1) in the first KB of the EBDA;
/// 2) in the last KB of system base memory;
/// 3) in the BIOS ROM between 0xE0000 and 0xFFFFF.
fn search() -> Option<*const Mp> {
    /// Look for an MP structure in the len bytes at addr.
    unsafe fn search1(addr: PAddr<u8>, len: usize) -> Option<*const Mp> {
        let mut addr = p2v(addr);
        let end = addr + len;
        while addr < end {
            let mp = addr.cast::<Mp>().ptr();
            if (*mp).verify() {
                return Some(mp);
            }
            addr += size_of::<Mp>();
        }
        None
    }

    // 0x40E (1 ward): EBDA base address >> 4 (usually!)
    unsafe {
        let ebda_addr = (*p2v(PAddr::<u16>::from_raw(0x40E)).ptr() as usize) << 4;
        let ebda: PAddr<u8> = PAddr::from_raw(ebda_addr);
        let first_try = if !ebda.is_null() {
            ebda
        } else {
            // 0x413 (1 word) : Number of kilobytes before EBDA
            let bef_ebda = (*p2v(PAddr::<u16>::from_raw(0x413)).ptr() as usize) * 1024;
            PAddr::from_raw(bef_ebda - 1024)
        };
        search1(first_try, 1024).or_else(|| search1(PAddr::from_raw(0xF0000), 0x10000))
    }
}

/// Search for an MP configuration table.
/// For now, don't accept the default configurations (conf_addr == 0).
/// Check for correct signature, calculate the checksum and,
/// if correct, check the version.
/// TODO: check extended table checksum.
fn config() -> Option<(*const Mp, *const MpConf)> {
    let mp = search()?;
    let conf = unsafe { (*mp).get_conf()? };
    match conf.version {
        1 | 4 => Some((mp, conf)),
        _ => None,
    }
}

pub fn init() {
    let (mp, conf) = config().expect("Expect to run on an SMP");

    let mut is_mp = true;
    let mut p = unsafe { conf.add(1) as *const u8 };
    let e = unsafe { (conf as *const u8).add((*conf).length as usize) };
    while p < e {
        let ty = unsafe { *p };
        let sz = match ty {
            proc_ent_type::MPPROC => unsafe {
                let pr = p as *const MpProc;
                if let Some(cpu) = super::proc::init_new_cpu() {
                    cpu.apic_id = (*pr).apic_id;
                    log!("cpu found (apic id: {})", (*pr).apic_id);
                } else {
                    log!("cpu ignored (apic id: {})", (*pr).apic_id);
                }
                size_of::<MpProc>()
            },
            proc_ent_type::MPIOAPIC => unsafe {
                let ioapic = p as *const MpIoApic;
                super::ioapic::IOAPIC_ID = (*ioapic).apic_no;
                size_of::<MpIoApic>()
            },
            proc_ent_type::MPBUS | proc_ent_type::MPIOINTR | proc_ent_type::MPLINTR => 8,
            _ => {
                is_mp = false;
                break;
            }
        };
        p = unsafe { p.add(sz) };
    }
    if !is_mp {
        panic!("Didn't find a suitable machine");
    }

    unsafe { super::lapic::LAPIC = Some((*conf).lapic_addr) };

    if unsafe { (*mp).imcr } != 0 {
        // Bochs doesn't support IMCR, so this doesn't run on Bochs.
        // But it would on real hardware.
        x86::outb(0x22, 0x70); // Select IMCR
        x86::outb(0x23, x86::inb(0x23) | 1); // Mask external interrupts
    }
}
