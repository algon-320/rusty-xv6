use utils::x86;

// I/O Addresses of the two programmable interrupt controllers
const IO_PIC1: u16 = 0x20; // Master (IRQs 0-7)
const IO_PIC2: u16 = 0xA0; // Slave (IRQs 8-15)

// Don't use the 8259A interrupt controllers.  Xv6 assumes SMP hardware.
pub fn init() {
    x86::outb(IO_PIC1 + 1, 0xFF);
    x86::outb(IO_PIC2 + 1, 0xFF);
}
