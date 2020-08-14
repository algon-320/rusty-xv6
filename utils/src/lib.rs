#![no_std]
#![feature(llvm_asm)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![allow(clippy::identity_op)]

pub mod address;
pub mod vga;
pub mod x86;

/// Imitate C99's designated initializer
#[macro_export]
macro_rules! assigned_array {
    ($default:expr; $len:expr; $([$idx:expr] = $val:expr),*) => {{
        let mut tmp = [$default; $len];
        $(tmp[$idx] = $val;)*
        tmp
    }};
}

#[macro_export]
macro_rules! print {
    ($color:expr;$($arg:tt)*) => {
        $crate::vga::_print_with_color($color, format_args!($($arg)*))
    };
    ($($arg:tt)*) => { $crate::vga::_print(format_args!($($arg)*)) };
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
            $crate::vga::LIGHT_GREEN;
            "[{}:{}]", core::file!(), core::line!())
    };
    ($val:expr) => {
        match $val {
            tmp =>{
                $crate::println!($crate::vga::LIGHT_GREEN;
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
            $crate::print_color::WHITE;
            "[{}:{}]", core::file!(), core::line!())
    };
    ($($arg:tt)*) => {
        $crate::println!(
            $crate::print_color::WHITE;
            "[{}:{}] {}", core::file!(), core::line!(), format_args!($($arg)*))
    };
}

pub mod print_color {
    pub use super::vga::{CYAN, LIGHT_GREEN, LIGHT_RED, WHITE, YELLOW};
}

pub mod prelude {
    pub use super::address::{PAddr, VAddr};
    pub use super::{dbg, log, print, print_color, println};
}
