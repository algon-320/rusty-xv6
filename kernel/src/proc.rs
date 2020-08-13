#[derive(Debug, Eq, PartialEq)]
pub struct Cpu {
    pub num_cli: i32,
    pub int_enabled: bool,
}

// TODO
pub static mut MAIN_CPU: Cpu = Cpu {
    num_cli: 0,
    int_enabled: false,
};

pub fn my_cpu() -> &'static mut Cpu {
    unsafe { &mut MAIN_CPU }
}
