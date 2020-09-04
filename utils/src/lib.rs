#![no_std]
#![feature(llvm_asm)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![allow(clippy::identity_op)]

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

pub mod prelude {
    pub use super::address::{PAddr, VAddr};
}
