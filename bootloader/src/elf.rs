/// ELF32 header
#[repr(C)]
pub struct ElfHeader {
    /// ELF Identification
    pub e_ident: [u8; 16],
    /// object file type
    pub e_type: u16,
    /// machine
    pub e_machine: u16,
    /// object file version
    pub e_version: u32,
    /// virtual entry point
    pub e_entry: extern "C" fn() -> (),
    /// program header table offset
    pub e_phoff: usize,
    /// section header table offset
    pub e_shoff: usize,
    /// processor-specific flags
    pub e_flags: u32,
    /// ELF header size
    pub e_ehsize: u16,
    /// program header entry size
    pub e_phentsize: u16,
    /// number of program header entries
    pub e_phnum: u16,
    /// section header entry size
    pub e_shent_size: u16,
    /// number of section header entries
    pub e_shnum: u16,
    /// section header table's "section header string table" entry offset
    pub e_shstrndx: u16,
}

/// ELF32 program header
#[repr(C)]
pub struct ProgHeader {
    /// segment type
    pub p_type: u32,
    /// segment offset
    pub p_offset: usize,
    /// virtual address of segment
    pub p_vaddr: *mut u8,
    /// physical address - ignored ?
    pub p_paddr: *mut u8,
    /// number of bytes in file for seg.
    pub p_filesz: usize,
    /// number of bytes in mem. for seg.
    pub p_memsz: usize,
    /// flags
    pub p_flags: u32,
    /// memory alignment
    pub p_align: usize,
}

const ELF_MAGIC: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]; // 0x7F, 'E', 'L', 'F'

impl ElfHeader {
    pub fn verify(&self) -> bool {
        self.e_ident[..4] == ELF_MAGIC
    }
    pub fn prog_headers(&self) -> &[ProgHeader] {
        let self_ptr = self as *const _ as *mut u8;
        let prg_hdr = unsafe { self_ptr.add(self.e_phoff) } as *mut ProgHeader;
        unsafe { core::slice::from_raw_parts(prg_hdr, self.e_phnum as usize) }
    }
}
