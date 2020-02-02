#![no_std]
#![no_main]

#[macro_use]
mod virt;

mod asm;
mod locked;
mod trap;

#[no_mangle]
pub extern "C" fn kernel_entry() -> ! {
    println!("Hello, world!");
    println!(
        "mhartid: {}, mvendorid: {}",
        asm::mhartid(),
        asm::mvendorid()
    );
    print!("Extensions available: ");
    let misa = asm::misa();
    let extensions = misa.extensions();

    for i in 0..26 {
        if (extensions >> i) & 1 == 1 {
            print!("{}", asm::EXTENSIONS[i]);
        }
    }

    println!();

    asm::ecall();

    panic!()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    #[allow(clippy::empty_loop)]
    loop {}
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!("we've aborted")
}
