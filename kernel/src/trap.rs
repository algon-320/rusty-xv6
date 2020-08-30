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

use super::memory::gate;
use super::memory::seg;
use utils::x86;

/// Interrupt descriptor table (shared by all CPUs).
static mut IDT: [gate::GateDesc; 256] = [gate::GateDesc::new(); 256];

// build.rs generates vector.S
global_asm!(include_str!(concat!(env!("OUT_DIR"), "/vectors.S")));
/* vector.S looks like:
    .globl alltraps
    .globl vector0
    vector0:
      pushl $0
      pushl $0
      jmp alltraps
    .globl vector1
    vector1:
      pushl $0
      pushl $1
      jmp alltraps
    ...

    .data
    .globl VECTORS
    VECTORS:
      .long vector0
      .long vector1
      .long vector2
      ...
*/
extern "C" {
    /// in vectors.S: array of 256 entry pointers
    static VECTORS: [u32; 256];
}

use super::lock::spin::SpinMutex;
static TICKS: SpinMutex<u32> = SpinMutex::new("time", 0);

pub fn init() {
    unsafe {
        for i in 0..256 {
            IDT[i].set(
                false,
                (seg::SEG_KCODE << 3) as u16,
                VECTORS[i] as *const u32 as u32,
                0,
            );
        }
        IDT[T_SYSCALL as usize].set(
            true,
            (seg::SEG_KCODE << 3) as u16,
            VECTORS[T_SYSCALL as usize] as *const u32 as u32,
            seg::dpl::USER,
        );
    }
}

/// Layout of the trap frame built on the stack by the
/// hardware and by alltraps, and passed to trap().
#[derive(Debug)]
#[repr(C)]
pub struct TrapFrame {
    // registers as pushed by pushal
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    orig_esp: u32, // useless & ignored
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,

    // rest of trap frame
    pub gs: u16,
    _padding1: u16,
    pub fs: u16,
    _padding2: u16,
    pub es: u16,
    _padding3: u16,
    pub ds: u16,
    _padding4: u16,
    pub trap_no: u32,

    // below here defined by x86 hardware
    pub err: u32,
    pub eip: usize,
    pub cs: u16,
    _padding5: u16,
    pub eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    pub esp: usize,
    pub ss: u16,
    _padding6: u16,
}

pub fn idt_init() {
    const IDT_SZ: usize = core::mem::size_of::<[gate::GateDesc; 256]>();
    unsafe { x86::lidt(IDT.as_ptr() as *const u8, IDT_SZ as u16) };
}

#[no_mangle]
pub extern "C" fn trap(trap_frame: *const TrapFrame) {
    // use super::proc::my_cpu_id;
    // utils::log!("[cpu:{}] trap", my_cpu_id());
    super::lapic::eoi();
}

extern "C" {
    pub fn trapret();
}

global_asm! {r#"
# vectors.S sends all traps here.
.globl alltraps
alltraps:
    # Build trap frame.
    pushl %ds
    pushl %es
    pushl %fs
    pushl %gs
    pushal

    # Set up data segments.
    movw $(2<<3), %ax  # SEG_KDATA<<3
    movw %ax, %ds
    movw %ax, %es

    # Call trap(tf), where tf=%esp
    pushl %esp
    call trap
    addl $4, %esp

# Return falls through to trapret...
.globl trapret
trapret:
    popal
    popl %gs
    popl %fs
    popl %es
    popl %ds
    addl $0x8, %esp  # trap_no and err
    iret             # pop %eip, %cs, %eflags
                     # (and also %esp, %ss when crossing rings)
                     # then return
"#}
