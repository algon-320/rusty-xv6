use utils::x86;

const COM1: u16 = 0x03F8;

static mut IS_UART: bool = false;

pub fn init() {
    // Turn off the FIFO
    x86::outb(COM1 + 2, 0);

    // 9600 baud, 8 data bits, 1 stop bit, parity off.
    x86::outb(COM1 + 3, 0x80);
    x86::outb(COM1 + 0, (115200 / 9600) as u8);
    x86::outb(COM1 + 1, 0);
    x86::outb(COM1 + 3, 0x03); // Lock divisor, 8 data bits.
    x86::outb(COM1 + 4, 0);
    x86::outb(COM1 + 1, 0x01); // Enable receive interrupts.

    // If status is 0xFF no serial port.
    if x86::inb(COM1 + 5) == 0xFF {
        return;
    }
    unsafe { IS_UART = true };

    // Acknowledge pre-existing interrupt conditions;
    // enable interrupts.
    x86::inb(COM1 + 2);
    x86::inb(COM1 + 0);
    super::ioapic::enable(super::trap::IRQ_COM1, 0);

    // Announce that we're here.
    puts("xv6...！\n");
}

fn puts(s: &str) {
    for c in s.as_bytes() {
        putc(*c);
    }
}

fn putc(c: u8) {
    if !unsafe { IS_UART } {
        return;
    }
    const RETRIES: usize = 128;
    for _ in 0..RETRIES {
        if (x86::inb(COM1 + 5) & 0x20) > 0 {
            break;
        }
        super::lapic::micro_delay(10);
    }
    x86::outb(COM1 + 0, c);
}
