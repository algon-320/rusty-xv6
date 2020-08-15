#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Cpu {
    /// Local APIC ID
    pub apic_id: u8,
    pub num_cli: i32,
    pub int_enabled: bool,
}
impl Cpu {
    pub const fn zero() -> Self {
        Self {
            apic_id: 0,
            num_cli: 0,
            int_enabled: false,
        }
    }
}

// TODO
pub static mut MAIN_CPU: Cpu = Cpu::zero();

pub fn my_cpu() -> &'static mut Cpu {
    unsafe { &mut MAIN_CPU }
}
