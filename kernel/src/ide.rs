use super::fs::Buf;
use super::lock::spin::SpinMutex;
use utils::prelude::*;
use utils::x86;

static IDE_QUEUE: SpinMutex<Option<&'static Buf>> = SpinMutex::new("ide", None);
static mut HAVE_DISK: bool = false;

const IDE_BSY: u8 = 0x80;
const IDE_DRDY: u8 = 0x40;
const IDE_DF: u8 = 0x20;
const IDE_ERR: u8 = 0x01;

const PORT_BASE: u16 = 0x1F0;

/// Wait for IDE disk to become ready.
fn wait(check_err: bool) -> Option<()> {
    let r = loop {
        let r = x86::inb(PORT_BASE + 7);
        if r & (IDE_BSY | IDE_DRDY) == IDE_DRDY {
            break r;
        }
    };
    if check_err && (r & (IDE_DF | IDE_ERR)) > 0 {
        None
    } else {
        Some(())
    }
}

pub fn init() {
    let last_cpu = super::proc::cpus().len() - 1;
    super::ioapic::enable(super::trap::IRQ_IDE, last_cpu);
    wait(false);

    // Check if disk 1 is present
    x86::outb(PORT_BASE + 6, 0xE0 | (1 << 4));
    for _ in 0..1000 {
        if x86::inb(PORT_BASE + 7) != 0 {
            unsafe { HAVE_DISK = true };
            break;
        }
    }
    unsafe { dbg!(HAVE_DISK) };
    // Switch back to disk 0
    x86::outb(PORT_BASE + 6, 0xE0 | (0 << 4));
}
