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

use super::spinlock::SpinMutex;
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
    edi: u32,
    esi: u32,
    ebp: u32,
    orig_esp: u32, // useless & ignored
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,

    // rest of trap frame
    gs: u16,
    _padding1: u16,
    fs: u16,
    _padding2: u16,
    es: u16,
    _padding3: u16,
    ds: u16,
    _padding4: u16,
    trap_no: u32,

    // below here defined by x86 hardware
    err: u32,
    eip: u32,
    cs: u16,
    _padding5: u16,
    eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    esp: u32,
    ss: u16,
    _padding6: u16,
}

#[no_mangle]
pub extern "C" fn trap(_trap_frame: *const TrapFrame) {
    todo!()
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
    addl $0x8, %esp  # trapno and errcode
    iret
"#}
