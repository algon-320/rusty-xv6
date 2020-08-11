#![no_std]
#![feature(llvm_asm)]

pub mod address;
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
