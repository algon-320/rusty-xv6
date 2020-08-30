/// Eflags register
pub mod eflags {
    /// Interrupt Enable
    pub const FL_IF: u32 = 0x00000200;
}

/// read a byte from the port
#[inline]
pub fn inb(port: u16) -> u8 {
    let data: u8;
    unsafe {
        llvm_asm!("inb $1, $0"
            : "={al}"(data)
            : "{dx}"(port)
            : 
            : "volatile");
    }
    data
}

/// read cnt double-words from the port
#[inline]
pub fn insl(port: u16, addr: *mut u32, cnt: usize) {
    let mut _addr = addr;
    let mut _cnt = cnt;
    unsafe {
        llvm_asm!("cld; rep insl"
            : "+{edi}"(_addr), "+{ecx}"(_cnt)
            : "{dx}"(port)
            : "memory", "cc"
            : "volatile");
    }
}

/// write the byte (data) to the port
#[inline]
pub fn outb(port: u16, data: u8) {
    unsafe {
        llvm_asm!("outb $0, $1"
            :
            : "{al}"(data), "{dx}"(port)
            :
            : "volatile");
    }
}

/// write the word (data) to the port
#[inline]
pub fn outw(port: u16, data: u16) {
    unsafe {
        llvm_asm!("outw $0, $1"
            :
            : "{ax}"(data), "{dx}"(port)
            :
            : "volatile");
    }
}

/// write cnt double-words from the addr to the port
#[inline]
pub fn outsl(port: u16, addr: *const u32, cnt: usize) {
    let mut _addr = addr;
    let mut _cnt = cnt;
    unsafe {
        llvm_asm!("cld; rep outsl"
            : "+{esi}"(_addr), "+{ecx}"(_cnt)
            : "{dx}"(port)
            : "cc"
            : "volatile");
    }
}

/// write the byte (data) to the address (cnt times repeatedly)
#[inline]
pub fn stosb(addr: *const u8, data: u8, cnt: usize) {
    let mut _addr = addr;
    let mut _cnt = cnt;
    unsafe {
        llvm_asm!("cld; rep stosb"
            : "+{edi}"(_addr), "+{ecx}"(_cnt)
            : "{al}"(data)
            : "memory", "cc"
            : "volatile");
    }
}

/// write the double word (data) to the address (cnt times repeatedly)
#[inline]
pub fn stosl(addr: *const u8, data: u32, cnt: usize) {
    let mut _addr = addr;
    let mut _cnt = cnt;
    unsafe {
        llvm_asm!("cld; rep stosl"
            : "+{edi}"(_addr), "+{ecx}"(_cnt)
            : "{eax}"(data)
            : "memory", "cc"
            : "volatile");
    }
}

/// write new_val to addr and return the old value
#[inline]
pub fn xchgl(addr: *mut u32, new_val: u32) -> u32 {
    let mut result;
    unsafe {
        llvm_asm!("lock; xchgl $0, $1"
            : "+*m"(addr), "={eax}"(result)
            : "1"(new_val)
            : "cc"
            : "volatile");
    }
    result
}

/// Return eflags
#[inline]
pub fn read_eflags() -> u32 {
    let mut eflags;
    unsafe {
        llvm_asm!("pushfl; popl $0"
            : "=r"(eflags)
            :
            :
            : "volatile");
    }
    eflags
}

#[inline]
pub fn cli() {
    unsafe {
        llvm_asm!("cli"::::"volatile");
    }
}
#[inline]
pub fn sti() {
    unsafe {
        llvm_asm!("sti"::::"volatile");
    }
}

/// write 0 to the memory specified by addr
#[inline]
pub fn movl0(addr: *mut u32) {
    unsafe {
        llvm_asm!("movl $$0, $0"
            : "+*m"(addr)
            :
            :
            : "volatile");
    }
}

#[inline]
pub fn lcr3(val: u32) {
    unsafe {
        llvm_asm!("movl $0, %cr3"
            :
            : "r"(val)
            :
            : "volatile");
    }
}

#[inline]
pub fn lgdt(seg_desc: *const u8, sz: u16) {
    let pd: [u16; 3] = [
        sz - 1,
        seg_desc as usize as u16,
        (seg_desc as usize).wrapping_shr(16) as u16,
    ];
    unsafe {
        llvm_asm!("lgdt ($0)"
            :
            : "r"(pd.as_ptr())
            :
            : "volatile");
    }
}
#[inline]
pub fn lidt(gate_desc: *const u8, sz: u16) {
    let pd: [u16; 3] = [
        sz - 1,
        gate_desc as usize as u16,
        (gate_desc as usize).wrapping_shr(16) as u16,
    ];
    unsafe {
        llvm_asm!("lidt ($0)"
            :
            : "r"(pd.as_ptr())
            :
            : "volatile");
    }
}

#[inline]
pub fn ltr(sel: u16) {
    unsafe {
        llvm_asm!("ltr $0"
            :
            : "r"(sel)
            :
            : "volatile");
    }
}

/// do nothing
#[inline]
pub fn nop() {
    unsafe {
        llvm_asm!("nop");
    }
}

pub const FL_IF: u32 = 0x00000200;
