use super::fs::inode::InodeRef;
use super::lock::spin::SpinMutex;
use super::memory::{pg_dir, seg};
use super::trap;
use core::cell::{RefCell, RefMut};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use utils::x86;

/// Task state segment format
#[repr(C)]
pub struct TaskState {
    /// Old ts selector
    pub link: u32,
    /// Stack pointers and segment selectors
    pub esp0: u32,
    /// after an increase in privilege level
    pub ss0: u16,
    _padding1: u16,
    pub esp1: *mut u32,
    pub ss1: u16,
    _padding2: u16,
    pub esp2: *mut u32,
    pub ss2: u16,
    _padding3: u16,
    /// Page directory base
    pub cr3: *mut u8,
    /// Saved state from last task switch
    pub eip: *mut u32,
    pub eflags: u32,
    /// More saved state (registers)
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esp: *mut u32,
    pub ebp: *mut u32,
    pub esi: u32,
    pub edi: u32,
    /// Even more saved state (segment selectors)
    pub es: u16,
    _padding4: u16,
    pub cs: u16,
    _padding5: u16,
    pub ss: u16,
    _padding6: u16,
    pub ds: u16,
    _padding7: u16,
    pub fs: u16,
    _padding8: u16,
    pub gs: u16,
    _padding9: u16,
    pub ldt: u16,
    _padding10: u16,
    /// Trap on task switch
    pub t: u16,
    /// I/O map base address
    pub iomb: u16,
}
impl TaskState {
    pub const fn zero() -> Self {
        use core::ptr::null_mut;
        Self {
            link: 0,
            esp0: 0,
            ss0: 0,
            _padding1: 0,
            esp1: null_mut(),
            ss1: 0,
            _padding2: 0,
            esp2: null_mut(),
            ss2: 0,
            _padding3: 0,
            cr3: null_mut(),
            eip: null_mut(),
            eflags: 0,
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: null_mut(),
            ebp: null_mut(),
            esi: 0,
            edi: 0,
            es: 0,
            _padding4: 0,
            cs: 0,
            _padding5: 0,
            ss: 0,
            _padding6: 0,
            ds: 0,
            _padding7: 0,
            fs: 0,
            _padding8: 0,
            gs: 0,
            _padding9: 0,
            ldt: 0,
            _padding10: 0,
            t: 0,
            iomb: 0,
        }
    }
}

pub struct Cpu {
    pub scheduler: *const Context,
    pub task_state: TaskState,
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
                scheduler: core::ptr::null(),
                task_state: TaskState::zero(),
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

/// Saved registers for kernel context switches.
/// Don't need to save all the segment registers (%cs, etc),
/// because they are constant across kernel contexts.
/// Don't need to save %eax, %ecx, %edx, because the
/// x86 convention is that the caller has saved them.
/// Contexts are stored at the bottom of the stack they
/// describe; the stack pointer is the address of the context.
/// The layout of the context matches the layout of the stack in swtch.S
/// at the "Switch stacks" comment. Switch doesn't save eip explicitly,
/// but it is on the stack and alloc_proc() manipulates it.
#[repr(C)]
pub struct Context {
    edi: u32,
    esi: u32,
    ebx: u32,
    ebp: u32,
    eip: u32,
}
impl Context {
    pub fn zero() -> Self {
        Self {
            edi: 0,
            esi: 0,
            ebx: 0,
            ebp: 0,
            eip: 0,
        }
    }
}

const MAX_NPROC: usize = 64;

#[derive(Debug, Eq, PartialEq)]
enum ProcessState {
    Unused,
    Embryo,
    Sleeping,
    Runnable,
    Running,
    Zombie,
}
pub struct Process {
    state: ProcessState,                // Process state
    size: usize,                        // Size of process memory (bytes)
    pg_dir: *mut pg_dir::PageDirectory, // Page table
    kernel_stack: *mut u8,              // Bottom of kernel stack for this process
    pid: u32,                           // Process ID
    trap_frame: *mut trap::TrapFrame,   // Trap frame for current syscall
    context: *mut Context,              // swtch() here to run process
    pub cwd: Option<InodeRef>,          // Current directory

    name: [u8; 16], // Process name (debugging)
}
impl Process {
    pub const fn zero() -> Self {
        Self {
            state: ProcessState::Unused,
            size: 0,
            pg_dir: core::ptr::null_mut(),
            kernel_stack: core::ptr::null_mut(),
            pid: u32::MAX,
            trap_frame: core::ptr::null_mut(),
            context: core::ptr::null_mut(),
            cwd: None,

            name: [0; 16],
        }
    }
}
impl core::fmt::Debug for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut name_len = self.name.len();
        for (i, c) in self.name.iter().enumerate() {
            if *c == b'\0' {
                name_len = i;
                break;
            }
        }

        f.debug_struct("Process")
            .field("state", &self.state)
            .field("pid", &self.pid)
            .field(
                "name",
                &core::str::from_utf8(&self.name[..name_len]).unwrap(),
            )
            .finish()
    }
}

struct ProcessTable {
    initialized: bool,
    table: [Option<*mut Process>; MAX_NPROC],
}
impl ProcessTable {
    pub fn init(&mut self) {
        assert!(!self.initialized);
        for p in unsafe { _PROC_ARENA.iter_mut() } {
            p.state = ProcessState::Unused;
            self.put(p as *mut _);
        }
        self.initialized = true;
    }

    pub fn put(&mut self, r: *mut Process) {
        let idx = unsafe { r.offset_from(_PROC_ARENA.as_ptr()) };
        assert!(self.table[idx as usize].replace(r).is_none());
    }

    /// Search unused process
    pub fn get_unused(&mut self) -> Option<*mut Process> {
        for slot in self.table.iter_mut() {
            if slot.is_some() {
                return slot.take();
            }
        }
        None
    }
}
unsafe impl Send for ProcessTable {}

static mut _PROC_ARENA: [Process; MAX_NPROC] = [Process::zero(); MAX_NPROC];
static PROC_TABLE: SpinMutex<ProcessTable> = SpinMutex::new(
    "ptable",
    ProcessTable {
        initialized: false,
        table: [None; MAX_NPROC],
    },
);
static mut INIT_PROC: *mut Process = core::ptr::null_mut();
static NEXT_PID: AtomicU32 = AtomicU32::new(1);

pub fn init() {
    PROC_TABLE.lock().init();
}

/// Disable interrupts so that we are not rescheduled
/// while reading proc from the cpu structure
pub fn my_proc() -> *mut Process {
    super::lock::push_cli();
    let p = my_cpu().current_proc;
    super::lock::pop_cli();
    p
}

/// Look in the process table for an UNUSED proc.
/// If found, change state to EMBRYO and initialize
/// state required to run in the kernel.
/// Otherwise return 0.
fn alloc_proc() -> Option<*mut Process> {
    let p = PROC_TABLE.lock().get_unused()?;
    unsafe {
        let p = &mut *p;
        p.state = ProcessState::Embryo;
        p.pid = NEXT_PID.fetch_add(1, Ordering::SeqCst);

        // Allocate kernel stack.
        p.kernel_stack = match super::kalloc::kalloc() {
            Some(page) => page as *mut _,
            None => {
                p.state = ProcessState::Unused;
                PROC_TABLE.lock().put(p);
                return None;
            }
        };
        let sp = p.kernel_stack.add(super::memory::KSTACKSIZE);
        use core::mem::size_of;

        // Leave room for trap frame.
        let sp = sp.sub(size_of::<trap::TrapFrame>());
        p.trap_frame = sp as *mut _;
        rlibc::memset(sp, 0, size_of::<trap::TrapFrame>());

        // Set up new context to start executing at forkret,
        // which returns to trapret.
        type FnPtr = unsafe extern "C" fn();
        let sp = sp.sub(size_of::<FnPtr>());
        {
            let fp = sp as *mut FnPtr;
            *fp = trap::trapret as FnPtr;
        }

        let sp = sp.sub(size_of::<Context>());
        {
            p.context = sp as *mut Context;
            let mut ctx = Context::zero();
            ctx.eip = forkret as usize as u32;
            *p.context = ctx;
        }
    }
    Some(p)
}

/// Set up first user process.
pub fn user_init() {
    use super::memory::PAGE_SIZE;
    use super::vm;

    const INIT_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/init.bin"));

    let p = alloc_proc().expect("user_init: out of memory");
    unsafe {
        let p = &mut *p;
        p.pg_dir = vm::setup_kvm().expect("user_init: out of memory");
        vm::uvm::init(&mut *p.pg_dir, INIT_CODE);
        p.size = PAGE_SIZE;
        {
            let tf = &mut *p.trap_frame;
            tf.cs = (seg::SEG_UCODE << 3) as u16 | seg::dpl::USER as u16;
            let udata = (seg::SEG_UDATA << 3) as u16 | seg::dpl::USER as u16;
            tf.ds = udata;
            tf.es = udata;
            tf.ss = udata;
            tf.eflags = x86::eflags::FL_IF;
            tf.esp = PAGE_SIZE;
            tf.eip = 0; // begin of init
        }
        let name = b"init\0";
        p.name[..name.len()].copy_from_slice(name);
        p.cwd = super::fs::inode::from_name("/");
        p.state = ProcessState::Runnable;

        utils::log!("init = {:?}", p);

        INIT_PROC = p;
        PROC_TABLE.lock().put(p);
    }
}

/// A fork child's very first scheduling by scheduler()
/// will swtch here.  "Return" to user space.
#[no_mangle]
extern "C" fn forkret() {
    todo!()
}
