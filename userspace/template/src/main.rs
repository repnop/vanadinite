#![no_std]
#![no_main]

#[no_mangle]
fn main() {
    libvanadinite::print("hello world\n");
    libvanadinite::print("this is print 2\n");
    libvanadinite::exit();
}
