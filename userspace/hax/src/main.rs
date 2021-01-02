#![no_std]
#![no_main]

#[no_mangle]
fn main() {
    libvanadinite::print(unsafe { core::slice::from_raw_parts(0xffffffd000004690 as *mut u8, 1024) });
    libvanadinite::print("this is print 2\n");
    libvanadinite::exit();
}
