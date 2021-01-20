fn main() {
    let mut input = [0; 10];
    let mut total_read = 0;

    while total_read < 10 {
        let start = total_read;
        let read = read_stdin(&mut input[start..]);
        total_read += read;
        print!("{}", core::str::from_utf8(&input[start..][..read]).unwrap());
    }

    print!("\nyou typed: ");
    println!("{}", core::str::from_utf8(&input).unwrap());

    let result = std::syscalls::print(unsafe { core::slice::from_raw_parts(0xffffffd000004690 as *mut u8, 1024) });
    println!("{:?}", result);

    let result = std::syscalls::print(&input[..]);
    println!("\n{:?}", result);
}
