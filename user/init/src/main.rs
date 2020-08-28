#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(global_asm)]

global_asm! {r#"
.code32
.global start
start:
    jmp start
"#}

#[panic_handler]
#[no_mangle]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[lang = "eh_personality"]
#[no_mangle]
fn eh_personality() -> ! {
    loop {}
}
