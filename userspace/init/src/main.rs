#![feature(asm)]
#![no_std]
#![no_main]

extern crate libvanadinite;

#[no_mangle]
fn main() {
    loop {
        unsafe { asm!("nop") };
    }
}
