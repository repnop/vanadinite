#![feature(asm)]

fn main() {
    loop {
        unsafe { asm!("nop") };
    }
}
