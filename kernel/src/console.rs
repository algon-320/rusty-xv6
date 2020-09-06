use super::fs::file::{init_dev, Dev, CONSOLE};
use super::fs::Result;

pub fn console_write(buf: &[u8]) -> Result<usize> {
    todo!()
}
pub fn console_read(buf: &mut [u8]) -> Result<usize> {
    todo!()
}

pub fn init() {
    let cons = Dev {
        read: Some(console_read),
        write: Some(console_write),
    };
    unsafe { init_dev(CONSOLE, cons) };
    super::ioapic::enable(super::trap::IRQ_KBD, 0);
}

pub mod vga {
    use crate::lock::spin::SpinMutex;
    use crate::uart;
    use core::ptr::{read_volatile, write_volatile};
    use utils::x86;

    // from: https://os.phil-opp.com/vga-text-mode/

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum Color {
        Black = 0,
        Blue = 1,
        Green = 2,
        Cyan = 3,
        Red = 4,
        Magenta = 5,
        Brown = 6,
        LightGray = 7,
        DarkGray = 8,
        LightBlue = 9,
        LightGreen = 10,
        LightCyan = 11,
        LightRed = 12,
        Pink = 13,
        Yellow = 14,
        White = 15,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct ColorCode(u8);
    impl ColorCode {
        pub const fn new(fg: Color, bg: Color) -> Self {
            ColorCode((bg as u8) << 4 | (fg as u8))
        }
    }

    /// fg: White, bg: Black
    pub const WHITE: ColorCode = ColorCode::new(Color::White, Color::Black);
    /// fg: LightRed, bg: Black
    pub const LIGHT_RED: ColorCode = ColorCode::new(Color::LightRed, Color::Black);
    /// fg: Yellow, bg: Black
    pub const YELLOW: ColorCode = ColorCode::new(Color::Yellow, Color::Black);
    /// fg: Cyan, bg: Black
    pub const CYAN: ColorCode = ColorCode::new(Color::LightCyan, Color::Black);
    /// fg: LightGreen, bg: Black
    pub const LIGHT_GREEN: ColorCode = ColorCode::new(Color::LightGreen, Color::Black);

    const HEIGHT: usize = 25;
    const WIDTH: usize = 80;
    const CRT_PORT: u16 = 0x3D4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(C)]
    struct ScreenCell {
        ascii: u8,
        color: ColorCode,
    }
    impl Default for ScreenCell {
        fn default() -> Self {
            Self {
                ascii: b' ',
                color: WHITE,
            }
        }
    }

    #[repr(transparent)]
    struct Buffer {
        cells: [[ScreenCell; WIDTH]; HEIGHT],
    }
    impl Buffer {
        fn write(&mut self, row: usize, col: usize, cell: ScreenCell) {
            let ptr = &mut self.cells[row][col] as *mut ScreenCell;
            unsafe { write_volatile(ptr, cell) };
        }
        fn read(&self, row: usize, col: usize) -> ScreenCell {
            let ptr = &self.cells[row][col] as *const ScreenCell;
            unsafe { read_volatile(ptr) }
        }
    }

    pub struct Writer {
        column_position: usize,
        row_position: usize,
        color: ColorCode,
        buffer: *mut Buffer,
    }
    unsafe impl Send for Writer {}

    static VGA_WRITER: SpinMutex<Writer> = SpinMutex::new(
        "vga",
        Writer {
            column_position: 0,
            row_position: 0,
            color: WHITE,
            buffer: 0x800B8000 as *mut Buffer,
        },
    );

    pub fn clear_screen() {
        VGA_WRITER.lock().clear_screen();
    }

    impl Writer {
        fn write_cell(&mut self, r: usize, c: usize, cell: ScreenCell) {
            unsafe { (*self.buffer).write(r, c, cell) }
        }
        fn read_cell(&mut self, r: usize, c: usize) -> ScreenCell {
            unsafe { (*self.buffer).read(r, c) }
        }
        pub fn write_byte(&mut self, byte: u8) {
            match byte {
                b'\n' => self.new_line(),
                byte => {
                    if self.column_position >= WIDTH {
                        self.new_line();
                    }
                    self.write_cell(
                        self.row_position,
                        self.column_position,
                        ScreenCell {
                            ascii: byte,
                            color: self.color,
                        },
                    );
                    self.column_position += 1;
                    self.update_cursor();
                }
            }
        }

        pub fn write_string(&mut self, s: &str) {
            for byte in s.bytes() {
                match byte {
                    0x20..=0x7E | b'\n' => self.write_byte(byte),
                    _ => self.write_byte(0xFE),
                }
            }
        }

        pub fn clear_screen(&mut self) {
            for r in 0..HEIGHT {
                self.clear_row(r);
            }
        }

        fn new_line(&mut self) {
            if self.row_position == HEIGHT - 1 {
                // scroll rows
                for row in 1..HEIGHT {
                    for col in 0..WIDTH {
                        let cell = self.read_cell(row, col);
                        self.write_cell(row - 1, col, cell);
                    }
                }
                self.clear_row(HEIGHT - 1);
            } else {
                self.row_position += 1;
            }
            self.column_position = 0;
            self.update_cursor();
        }

        fn clear_row(&mut self, row: usize) {
            let blank = ScreenCell {
                ascii: b' ',
                color: self.color,
            };
            for col in 0..WIDTH {
                self.write_cell(row, col, blank);
            }
        }

        fn update_cursor(&self) {
            // from https://wiki.osdev.org/Text_Mode_Cursor
            let pos = self.row_position * WIDTH + self.column_position;
            x86::outb(CRT_PORT + 0, 0x0F);
            x86::outb(CRT_PORT + 1, (pos & 0xFF) as u8);
            x86::outb(CRT_PORT + 0, 0x0E);
            x86::outb(CRT_PORT + 1, ((pos >> 8) & 0xFF) as u8);
        }

        fn change_color(&mut self, color: ColorCode) {
            self.color = color;
        }
    }

    use core::fmt::Write;

    impl Write for Writer {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            uart::puts(s);
            self.write_string(s);
            Ok(())
        }
    }

    #[doc(hidden)]
    pub fn _print(args: core::fmt::Arguments) {
        let mut writer = VGA_WRITER.lock();
        writer.write_fmt(args).unwrap();
    }
    #[doc(hidden)]
    pub fn _print_with_color(color: ColorCode, args: core::fmt::Arguments) {
        let mut writer = VGA_WRITER.lock();
        writer.change_color(color);
        writer.write_fmt(args).unwrap();
        writer.change_color(WHITE);
    }
}

#[macro_export]
macro_rules! print {
    ($color:expr;$($arg:tt)*) => {
        $crate::console::vga::_print_with_color($color, format_args!($($arg)*))
    };
    ($($arg:tt)*) => { $crate::console::vga::_print(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($color:expr;$($arg:tt)*) => {
        $crate::print!($color; "{}\n", format_args!($($arg)*))
    };
    ($($arg:tt)*) => { $crate::print!("{}\n", format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! dbg {
    () => {
        $crate::println!(
            $crate::console::vga::LIGHT_GREEN;
            "[{}:{}]", core::file!(), core::line!())
    };
    ($val:expr) => {
        match $val {
            tmp =>{
                $crate::println!($crate::console::vga::LIGHT_GREEN;
                    "[{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), tmp);
                tmp
            }
        }
    };
    ($val:expr,) => { $crate::dbg!($val) };
    ($($val:expr),+ $(,)?) => { ($($crate::dbg!($val)),+,) };
}

#[macro_export]
macro_rules! log {
    () => {
        $crate::println!(
            $crate::console::print_color::WHITE;
            "[{}:{}]", core::file!(), core::line!())
    };
    ($($arg:tt)*) => {
        $crate::println!(
            $crate::console::print_color::WHITE;
            "[{}:{}] {}", core::file!(), core::line!(), format_args!($($arg)*))
    };
}

pub mod print_color {
    pub use super::vga::{CYAN, LIGHT_GREEN, LIGHT_RED, WHITE, YELLOW};
}
