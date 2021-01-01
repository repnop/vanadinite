#![feature(asm)]
#![no_std]
#![no_main]

#[no_mangle]
fn main() {
    loop {
        unsafe { asm!("nop") };
    }

    libvanadinite::exit();
}
