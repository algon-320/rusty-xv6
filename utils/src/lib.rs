#![no_std]
#![feature(llvm_asm)]
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
    ($color:expr;$($arg:tt)*) => ($crate::vga::_print_with_color($color, format_args!($($arg)*)));
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($color:expr;$($arg:tt)*) => ($crate::print!($color;"{}\n", format_args!($($arg)*)));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! dbg {
    () => {$crate::println!($crate::vga::LIGHT_GREEN; "[{}:{}]", core::file!(), core::line!())};
    ($val:expr) => {
        match $val {
            tmp =>{
                $crate::println!($crate::vga::LIGHT_GREEN; "[{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), tmp);
                tmp
            }
        }
    };
    ($val:expr,) => { $crate::dbg!($val) };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}
